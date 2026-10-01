"use strict";

const $ = (id) => document.getElementById(id);
let inventory = null;
let busy = false;
let groupedView = true;
let recentRequest = 0;
const removingRepositories = new Set();
// Starred skills by case-insensitive repository and exact path.
let starred = new Map();
let starredEnabled = true;
let starredRequest = 0;
const pendingStars = new Set();

function node(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

function externalLink(label, href, className) {
  const link = node("a", className, label);
  // Only source links generated for GitHub are navigable, even if data is corrupt.
  const url = new URL(href);
  if (url.origin !== "https://github.com" || url.username || url.password) {
    throw new Error("The scan returned an invalid source link. Please try again.");
  }
  link.href = url.href;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  return link;
}

function setBusy(value) {
  busy = value;
  $("scan-button").disabled = value;
  $("repository").disabled = value;
  document.querySelectorAll("[data-repository]").forEach((button) => { button.disabled = value; });
  document.querySelectorAll("[data-remove-repository]").forEach((button) => {
    button.disabled = value || removingRepositories.has(button.dataset.removeRepository);
  });
  $("scan-button-text").textContent = value ? "Scanning…" : "Scan repository";
  $("results-section").setAttribute("aria-busy", String(value));
  $("progress-panel").hidden = !value;
}

function selectRepository(repository) {
  if (busy) return;
  $("repository").value = repository;
  $("repository").focus();
}

function showRecentError(message) {
  $("recent-error").textContent = message;
  $("recent-error").hidden = false;
}

async function refreshRecent() {
  const request = ++recentRequest;
  try {
    const response = await fetch("/api/recent", { cache: "no-store" });
    if (!response.ok) throw new Error("Could not load recent repositories. Try refreshing the list.");
    const result = await response.json();
    if (request !== recentRequest) return;
    const list = document.createDocumentFragment();
    for (const recent of result.repositories) {
      const row = node("li", "recent-row");
      const select = node("button", "recent-repository", recent.repository);
      select.type = "button";
      select.dataset.repository = recent.repository;
      select.disabled = busy;
      select.setAttribute("aria-label", `Use ${recent.repository} for another scan`);
      select.addEventListener("click", () => selectRepository(recent.repository));
      const details = node("div", "recent-details");
      details.append(select);
      const scannedAt = new Date(recent.scanned_at);
      const summary = node("p", "recent-hint", `${recent.skill_count} skill${recent.skill_count === 1 ? "" : "s"} · Last scanned `);
      const time = node("time", "", scannedAt.toLocaleString());
      time.dateTime = scannedAt.toISOString();
      summary.append(time);
      details.append(summary);
      const remove = node("button", "text-button", "Remove");
      remove.type = "button";
      remove.dataset.removeRepository = recent.repository;
      remove.disabled = busy || removingRepositories.has(recent.repository);
      remove.setAttribute("aria-label", `Remove ${recent.repository} from recent repositories`);
      remove.addEventListener("click", () => removeRecent(recent.repository));
      row.append(details, remove);
      list.append(row);
    }
    $("recent-list").replaceChildren(list);
    $("recent-status").textContent = !result.enabled
      ? "Recent repositories are disabled on this server."
      : result.repositories.length ? "Most recently scanned first." : "No recently scanned repositories yet.";
    return true;
  } catch (error) {
    if (request !== recentRequest) return;
    $("recent-status").textContent = "Recent repositories are unavailable.";
    showRecentError(error instanceof TypeError ? "Could not connect to the local server. Retry when it is running." : error.message);
  }
}

async function removeRecent(repository) {
  if (busy || removingRepositories.has(repository)) return;
  removingRepositories.add(repository);
  setBusy(busy);
  $("recent-error").hidden = true;
  try {
    const response = await fetch("/api/recent/remove", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Skill-Scanner": "1" },
      body: JSON.stringify({ repository }),
      cache: "no-store",
    });
    if (!response.ok) throw new Error("Could not remove the repository. Try again.");
    if (await refreshRecent()) {
      $("recent-status").textContent = `Removed ${repository} from recent repositories.`;
    }
    $("refresh-recent").focus();
  } catch (error) {
    showRecentError(error instanceof TypeError ? "Could not connect to the local server. Retry when it is running." : error.message);
  } finally {
    removingRepositories.delete(repository);
    setBusy(busy);
  }
}

function starKey(repository, path) {
  return `${repository.toLowerCase()}\n${path}`;
}

function connectionMessage(error) {
  return error instanceof TypeError ? "Could not connect to the local server. Retry when it is running." : error.message;
}

function showStarredError(message) {
  $("starred-error").textContent = message;
  $("starred-error").hidden = false;
}

function updateStarButtons() {
  document.querySelectorAll("[data-star-path]").forEach((button) => {
    const key = starKey(inventory.repository, button.dataset.starPath);
    const pressed = starred.has(key);
    button.setAttribute("aria-pressed", String(pressed));
    // Disabling the button would drop keyboard focus; setStar ignores repeats.
    button.setAttribute("aria-disabled", String(pendingStars.has(key)));
    button.replaceChildren(node("span", "star-icon", pressed ? "★" : "☆"), pressed ? "Starred" : "Star");
    button.firstChild.setAttribute("aria-hidden", "true");
  });
  document.querySelectorAll("[data-unstar-key]").forEach((button) => {
    button.disabled = pendingStars.has(button.dataset.unstarKey);
  });
}

// Keep keyboard focus nearby when a filter change removes the focused card.
function rerenderSkills(fallbackFocus) {
  const focused = document.activeElement;
  const inResults = $("skill-list").contains(focused);
  renderSkills();
  if (inResults && !focused.isConnected) fallbackFocus.focus();
}

async function refreshStarred() {
  const request = ++starredRequest;
  try {
    const response = await fetch("/api/starred", { cache: "no-store" });
    if (!response.ok) throw new Error("Could not load starred skills. Refresh the page to try again.");
    const result = await response.json();
    if (request !== starredRequest) return;
    const previous = [...starred.keys()].sort().join("\0");
    starred = new Map(result.skills.map((skill) => [starKey(skill.repository, skill.path), skill]));
    const list = document.createDocumentFragment();
    for (const [key, skill] of starred) {
      const row = node("li", "recent-row starred-row");
      const details = node("div", "recent-details");
      details.append(node("p", "starred-name", skill.name), node("p", "recent-hint", `${skill.repository} · ${skill.path}`));
      const actions = node("div", "starred-actions");
      const link = externalLink("Source ↗", skill.link, "text-button");
      link.setAttribute("aria-label", `View ${skill.path} in ${skill.repository} on GitHub (opens in a new tab)`);
      const unstar = node("button", "text-button", "Unstar");
      unstar.type = "button";
      unstar.dataset.unstarKey = key;
      unstar.setAttribute("aria-label", `Unstar ${skill.path} in ${skill.repository}`);
      unstar.addEventListener("click", () => setStar(skill, skill.repository, null, false));
      actions.append(link, unstar);
      row.append(details, actions);
      list.append(row);
    }
    $("starred-list").replaceChildren(list);
    $("starred-status").textContent = !result.enabled
      ? "Starred skills are disabled on this server."
      : result.skills.length ? "Most recently starred first." : "No starred skills yet.";
    const changed = starredEnabled !== result.enabled ||
      ($("starred-only").checked && previous !== [...starred.keys()].sort().join("\0"));
    starredEnabled = result.enabled;
    $("starred-filter").hidden = !starredEnabled;
    if (!starredEnabled) $("starred-only").checked = false;
    if (changed) rerenderSkills($("starred-only"));
    else if (inventory) updateStarButtons();
    return true;
  } catch (error) {
    if (request !== starredRequest) return;
    $("starred-status").textContent = "Starred skills are unavailable.";
    showStarredError(connectionMessage(error));
  }
}

async function setStar(skill, repository, commit, adding) {
  const key = starKey(repository, skill.path);
  if (pendingStars.has(key)) return;
  const fromList = $("starred-list").contains(document.activeElement);
  pendingStars.add(key);
  if (inventory) updateStarButtons();
  $("starred-error").hidden = true;
  try {
    const response = await fetch(adding ? "/api/starred/add" : "/api/starred/remove", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Skill-Scanner": "1" },
      // The server keeps at most 200 characters of a name.
      body: JSON.stringify(adding
        ? { repository, path: skill.path, name: Array.from(skill.name).slice(0, 200).join(""), commit }
        : { repository, path: skill.path }),
      cache: "no-store",
    });
    if (!response.ok) {
      const body = await response.json().catch(() => ({}));
      throw new Error(body.message || `Could not ${adding ? "star" : "unstar"} the skill. Try again.`);
    }
    if (await refreshStarred()) {
      $("starred-status").textContent = `${adding ? "Starred" : "Unstarred"} ${skill.name}.`;
    }
  } catch (error) {
    showStarredError(connectionMessage(error));
  } finally {
    pendingStars.delete(key);
    if (inventory) updateStarButtons();
    if (fromList && !$("starred-list").contains(document.activeElement)) $("starred-title").focus();
  }
}

function showError(message) {
  inventory = null;
  $("scan-results").hidden = true;
  $("initial-state").hidden = false;
  $("results-status").textContent = "Scan incomplete";
  $("error-message").textContent = message;
  $("error").hidden = false;
  $("error").focus();
}

function skillCard(skill, heading = "h3") {
  const card = node("article", "skill-card");
  const title = node("div", "skill-heading");
  title.append(node(heading, "", skill.name));
  if (starredEnabled) {
    const star = node("button", "star-button");
    star.type = "button";
    star.dataset.starPath = skill.path;
    star.setAttribute("aria-label", `Star ${skill.path}`);
    star.addEventListener("click", () => {
      setStar(skill, inventory.repository, inventory.commit, !starred.has(starKey(inventory.repository, skill.path)));
    });
    title.append(star);
  }
  card.append(title);
  card.append(node("p", "skill-description", skill.description || "No description available."));
  card.append(node("code", "skill-path", skill.path));
  const link = externalLink("View on GitHub ↗", skill.link, "skill-link");
  link.setAttribute("aria-label", `View ${skill.path} on GitHub (opens in a new tab)`);
  card.append(link);
  if (skill.warnings.length) {
    const details = node("details", "skill-warning");
    const count = skill.warnings.length;
    details.append(node("summary", "", `${count} metadata warning${count === 1 ? "" : "s"}`));
    const list = node("ul");
    for (const warning of skill.warnings) {
      list.append(node("li", "", `${warning.field}: ${warning.message}`));
    }
    details.append(list);
    card.append(details);
  }
  return card;
}

function percentage(count) {
  const total = inventory.aggregation.statistics.total_skills;
  return `${total ? (100 * count / total).toFixed(1) : "0.0"}%`;
}

function groupCard(group, visibleIndices, filtered) {
  if (group.skill_indices.length === 1) return skillCard(inventory.skills[visibleIndices[0]]);
  const first = inventory.skills[group.skill_indices[0]];
  const card = node("article", "skill-group");
  const heading = node("div", "group-heading");
  heading.append(node("h3", "", first.name));
  heading.append(node("span", "group-size", `${group.skill_indices.length} skills`));
  card.append(heading);
  card.append(node("p", "group-statistics", `${percentage(group.skill_indices.length)} of all skills · ${group.skills_with_warnings} with warnings`));
  const details = node("details", "group-details");
  const count = visibleIndices.length;
  const label = count === group.skill_indices.length
    ? `View ${count} skills`
    : `Showing ${count} of ${group.skill_indices.length} skills matching filters`;
  details.append(node("summary", "", label));
  details.open = filtered;
  const members = node("div", "group-members");
  visibleIndices.forEach((index) => members.append(skillCard(inventory.skills[index], "h4")));
  details.append(members);
  card.append(details);
  return card;
}

function setView(grouped) {
  groupedView = grouped;
  $("grouped-view").setAttribute("aria-pressed", String(grouped));
  $("all-view").setAttribute("aria-pressed", String(!grouped));
  renderSkills();
}

function renderSkills() {
  if (!inventory) return;
  const query = $("search").value.trim().toLocaleLowerCase();
  const warningsOnly = $("warnings-only").checked;
  const similarOnly = $("similar-only").checked;
  const starredOnly = starredEnabled && $("starred-only").checked;
  const groups = inventory.aggregation.groups;
  const visible = new Set();
  const matchingGroups = [];
  for (const group of groups) {
    if (similarOnly && group.skill_indices.length < 2) continue;
    const indices = group.skill_indices.filter((index) => {
      const skill = inventory.skills[index];
      return (!warningsOnly || skill.warnings.length > 0) &&
        (!starredOnly || starred.has(starKey(inventory.repository, skill.path))) &&
        [skill.name, skill.description, skill.path].some((text) => text.toLocaleLowerCase().includes(query));
    });
    if (indices.length) {
      matchingGroups.push({ group, indices });
      indices.forEach((index) => visible.add(index));
    }
  }
  const cards = document.createDocumentFragment();
  if (groupedView) {
    matchingGroups.forEach(({ group, indices }) => cards.append(groupCard(group, indices, Boolean(query || warningsOnly || starredOnly))));
  } else {
    inventory.skills.forEach((skill, index) => { if (visible.has(index)) cards.append(skillCard(skill)); });
  }
  $("skill-list").classList.toggle("grouped", groupedView);
  $("skill-list").replaceChildren(cards);
  updateStarButtons();
  $("result-count").textContent = `Showing ${visible.size} of ${inventory.skills.length} skills` + (groupedView
    ? ` in ${matchingGroups.length} of ${groups.length} groups (including standalone skills) · largest first`
    : " · sorted by path");
  $("no-results").hidden = visible.size > 0;
  $("clear-filters").hidden = inventory.skills.length === 0;
  if (!visible.size) {
    if (inventory.skills.length) {
      $("empty-title").textContent = "No skills match your filters";
      $("empty-description").textContent = "Try another search, or clear the filters to see all discovered skills.";
    } else {
      $("empty-title").textContent = inventory.commit ? "No SKILL.md files found" : "This repository is empty";
      $("empty-description").textContent = inventory.commit
        ? "The scan completed successfully. There are no regular SKILL.md files on this repository’s default branch."
        : "The scan completed successfully. There is no commit to scan yet. Try another public repository.";
    }
  }
}

function renderInventory(result) {
  inventory = result;
  $("result-repository").textContent = result.repository;
  $("skill-count").textContent = result.skills.length;
  const stats = result.aggregation.statistics;
  $("warning-count").textContent = stats.skills_with_warnings;
  $("similar-group-count").textContent = stats.similar_groups;
  $("grouped-skill-count").textContent = stats.grouped_skills;
  $("grouped-skill-share").textContent = `${percentage(stats.grouped_skills)} of all skills`;
  $("standalone-count").textContent = stats.standalone_skills;
  $("largest-group-count").textContent = stats.largest_group;
  $("result-commit").replaceChildren();
  if (result.commit) {
    const link = externalLink(`Commit ${result.commit.slice(0, 7)} ↗`, `https://github.com/${result.repository}/commit/${result.commit}`);
    link.title = result.commit;
    link.setAttribute("aria-label", `Scanned commit ${result.commit} on GitHub (opens in a new tab)`);
    $("result-commit").append(link);
  } else {
    $("result-commit").textContent = "No commit to scan";
  }
  $("result-controls").hidden = result.skills.length === 0;
  $("results-status").textContent = "Scan complete";
  renderSkills();
  $("initial-state").hidden = true;
  $("scan-results").hidden = false;
}

function updateProgress(event) {
  $("progress-message").textContent = event.message;
  $("progress-count").textContent = event.total ? `${event.current} / ${event.total}` : "";
  if (event.total) {
    $("scan-progress").max = event.total;
    // Progress is emitted before downloading the named skill.
    $("scan-progress").value = Math.max(0, event.current - 1);
  } else {
    $("scan-progress").removeAttribute("value");
  }
}

async function readScan(response) {
  if (!response.body) throw new Error("This browser cannot stream scan results. Try a current browser.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let pending = "";
  let completed = false;
  function consume(line) {
    if (!line.trim()) return;
    const event = JSON.parse(line);
    if (event.type === "error") throw new Error(event.message);
    if (event.type === "progress") updateProgress(event);
    if (event.type === "history_warning") showRecentError(event.message);
    if (event.type === "complete") {
      renderInventory(event.inventory);
      completed = true;
    }
  }
  try {
    while (true) {
      const { value, done } = await reader.read();
      pending += decoder.decode(value, { stream: !done });
      let boundary;
      while ((boundary = pending.indexOf("\n")) >= 0) {
        consume(pending.slice(0, boundary));
        pending = pending.slice(boundary + 1);
      }
      if (done) break;
    }
    if (pending.trim()) consume(pending);
    if (!completed) throw new Error("The connection closed before the scan finished. Check that the local server is running, then retry.");
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

$("scan-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (busy) return;
  const repository = $("repository").value.trim();
  if (!repository) { $("repository").focus(); return; }
  inventory = null;
  $("error").hidden = true;
  $("recent-error").hidden = true;
  $("scan-results").hidden = true;
  $("initial-state").hidden = true;
  $("search").value = "";
  $("warnings-only").checked = false;
  $("similar-only").checked = false;
  $("starred-only").checked = false;
  setView(true);
  $("results-status").textContent = "Scanning repository…";
  updateProgress({ message: "Connecting to GitHub…" });
  setBusy(true);
  try {
    const response = await fetch("/api/scan", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Skill-Scanner": "1" },
      body: JSON.stringify({ repository }),
      cache: "no-store",
    });
    if (!response.ok) {
      const body = await response.json();
      throw new Error(body.message || "The scan could not start. Please try again.");
    }
    await readScan(response);
  } catch (error) {
    const message = error instanceof TypeError
      ? "Could not connect to the local server. Make sure skill-scanner serve is running, then retry."
      : error instanceof SyntaxError
        ? "The server returned an incomplete response. Retry the scan."
        : error.message;
    showError(message);
  } finally {
    setBusy(false);
    await refreshRecent();
  }
});

document.querySelectorAll("[data-repository]").forEach((button) => {
  button.addEventListener("click", () => {
    selectRepository(button.dataset.repository);
  });
});
$("search").addEventListener("input", renderSkills);
$("warnings-only").addEventListener("change", renderSkills);
$("similar-only").addEventListener("change", renderSkills);
$("starred-only").addEventListener("change", renderSkills);
$("grouped-view").addEventListener("click", () => setView(true));
$("all-view").addEventListener("click", () => setView(false));
$("clear-filters").addEventListener("click", () => {
  $("search").value = "";
  $("warnings-only").checked = false;
  $("similar-only").checked = false;
  $("starred-only").checked = false;
  renderSkills();
  $("search").focus();
});

$("refresh-recent").addEventListener("click", () => {
  $("recent-error").hidden = true;
  refreshRecent();
});
window.addEventListener("focus", () => {
  if (!busy) refreshRecent();
  refreshStarred();
});
refreshRecent();
refreshStarred();
