"use strict";

const $ = (id) => document.getElementById(id);
let inventory = null;
let busy = false;
let groupedView = true;
let organizationMode = false;
let recentRequest = 0;
const removingRepositories = new Set();
// Starred skills by case-insensitive repository and exact path.
let starred = new Map();
let starredEnabled = true;
let starredRequest = 0;
const pendingStars = new Set();
// Codex installations by the same key as stars.
let codexSkills = new Map();
let codexEnabled = true;
let codexRequest = 0;
const pendingCodex = new Set();

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
  $("repository-mode").disabled = value;
  $("organization-mode").disabled = value;
  document.querySelectorAll("[data-repository],[data-organization]").forEach((button) => { button.disabled = value; });
  document.querySelectorAll("[data-remove-repository]").forEach((button) => {
    button.disabled = value || removingRepositories.has(button.dataset.removeRepository);
  });
  $("scan-button-text").textContent = value ? "Scanning…" : organizationMode ? "Scan organization" : "Scan repository";
  $("results-section").setAttribute("aria-busy", String(value));
  $("progress-panel").hidden = !value;
}

function setMode(organization) {
  if (busy) return;
  organizationMode = organization;
  $("repository-mode").setAttribute("aria-pressed", String(!organization));
  $("organization-mode").setAttribute("aria-pressed", String(organization));
  $("scan-title").textContent = organization ? "Start with an organization" : "Start with a repository";
  $("target-label").textContent = organization ? "GitHub organization or user" : "GitHub repository";
  $("repository").placeholder = organization
    ? "organization or https://github.com/organization"
    : "owner/repository or https://github.com/owner/repository";
  $("repository").setAttribute("aria-describedby", organization ? "organization-hint" : "repository-hint");
  $("repository-hint").hidden = organization;
  $("organization-hint").hidden = !organization;
  $("repository-examples").hidden = organization;
  $("organization-examples").hidden = !organization;
  $("scan-button-text").textContent = organization ? "Scan organization" : "Scan repository";
}

function selectRepository(repository) {
  if (busy) return;
  setMode(false);
  $("repository").value = repository;
  $("repository").focus();
}

function selectOrganization(organization) {
  if (busy) return;
  setMode(true);
  $("repository").value = organization;
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

function skillKey(repository, path) {
  return `${repository.toLowerCase()}\n${path}`;
}

// Stars and installations address a repository-relative path at a scanned
// commit. Organization results prefix each path with owner/repository.
function skillSource(skill) {
  let source = { repository: inventory.repository, path: skill.path, commit: inventory.commit };
  if (inventory.organization !== undefined) {
    const [owner, name, ...rest] = skill.path.split("/");
    const repository = `${owner}/${name}`;
    const commit = inventory.repositories.find((summary) => summary.repository === repository)?.commit;
    source = { repository, path: rest.join("/"), commit };
  }
  return { ...source, key: skillKey(source.repository, source.path) };
}

function connectionMessage(error) {
  return error instanceof TypeError ? "Could not connect to the local server. Retry when it is running." : error.message;
}

function showStarredError(message) {
  $("starred-error").textContent = message;
  $("starred-error").hidden = false;
}

function updateStarButtons() {
  document.querySelectorAll("[data-star-key]").forEach((button) => {
    const key = button.dataset.starKey;
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
    starred = new Map(result.skills.map((skill) => [skillKey(skill.repository, skill.path), skill]));
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
      unstar.addEventListener("click", () => setStar(skill, skill, false));
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

async function setStar(skill, source, adding) {
  const { repository, path, commit } = source;
  const key = skillKey(repository, path);
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
        ? { repository, path, name: Array.from(skill.name).slice(0, 200).join(""), commit }
        : { repository, path }),
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

function showCodexError(message) {
  $("codex-error").textContent = message;
  $("codex-error").hidden = false;
}

function updateCodexButtons() {
  document.querySelectorAll("[data-codex-key]").forEach((button) => {
    const key = button.dataset.codexKey;
    const installed = codexSkills.has(key);
    const pending = pendingCodex.has(key);
    button.dataset.installed = String(installed);
    // Disabling the button would drop keyboard focus; setCodex ignores repeats.
    button.setAttribute("aria-disabled", String(pending));
    button.textContent = pending
      ? installed ? "Removing…" : "Installing…"
      : installed ? "Remove from Codex" : "Install for Codex";
    button.setAttribute("aria-label", installed
      ? `Remove ${button.dataset.codexPath} from Codex`
      : `Install ${button.dataset.codexPath} for Codex`);
  });
  document.querySelectorAll("[data-codex-remove-key]").forEach((button) => {
    button.disabled = pendingCodex.has(button.dataset.codexRemoveKey);
  });
}

async function refreshCodex() {
  const request = ++codexRequest;
  try {
    const response = await fetch("/api/codex", { cache: "no-store" });
    if (!response.ok) throw new Error("Could not load Codex skills. Refresh the page to try again.");
    const result = await response.json();
    if (request !== codexRequest) return;
    codexSkills = new Map(result.skills.map((skill) => [skillKey(skill.repository, skill.path), skill]));
    const list = document.createDocumentFragment();
    for (const [key, skill] of codexSkills) {
      const row = node("li", "recent-row starred-row");
      const details = node("div", "recent-details");
      details.append(node("p", "starred-name", skill.name), node("p", "recent-hint", `${skill.repository} · ${skill.path}`));
      const actions = node("div", "starred-actions");
      const link = externalLink("Source ↗", skill.link, "text-button");
      link.setAttribute("aria-label", `View the installed ${skill.path} from ${skill.repository} on GitHub (opens in a new tab)`);
      const remove = node("button", "text-button", "Remove");
      remove.type = "button";
      remove.dataset.codexRemoveKey = key;
      remove.setAttribute("aria-label", `Remove ${skill.name} from Codex`);
      remove.addEventListener("click", () => setCodex(skill, { ...skill, key }, false));
      actions.append(link, remove);
      row.append(details, actions);
      list.append(row);
    }
    $("codex-list").replaceChildren(list);
    if (result.enabled) {
      const directory = node("code", "", result.directory);
      $("codex-hint").replaceChildren("Installed in ", directory, ", where Codex finds personal skills. Restart Codex if a new skill doesn’t appear.");
    } else {
      $("codex-hint").textContent = "Install skills from the results to use them in Codex.";
    }
    $("codex-status").textContent = !result.enabled
      ? "Codex skill installation is disabled on this server."
      : result.skills.length ? "Installed by Skill Scanner, sorted by folder name." : "No skills installed for Codex yet.";
    const changed = codexEnabled !== result.enabled;
    codexEnabled = result.enabled;
    if (changed) rerenderSkills($("codex-title"));
    else if (inventory) updateCodexButtons();
    return true;
  } catch (error) {
    if (request !== codexRequest) return;
    $("codex-status").textContent = "Codex skills are unavailable.";
    showCodexError(connectionMessage(error));
  }
}

// Install the scanned commit so Codex gets the files shown in the results.
async function setCodex(skill, source, installing) {
  const { repository, path, commit, key } = source;
  if (pendingCodex.has(key)) return;
  const name = installing ? skill.name : codexSkills.get(key)?.name;
  if (!installing && !name) return;
  const fromList = $("codex-list").contains(document.activeElement);
  pendingCodex.add(key);
  updateCodexButtons();
  $("codex-error").hidden = true;
  $("codex-status").textContent = installing ? `Installing ${skill.name} for Codex…` : `Removing ${name} from Codex…`;
  try {
    const response = await fetch(installing ? "/api/codex/install" : "/api/codex/remove", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Skill-Scanner": "1" },
      body: JSON.stringify(installing ? { repository, path, commit } : { name }),
      cache: "no-store",
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      throw new Error(body.message || `Could not ${installing ? "install" : "remove"} the skill. Try again.`);
    }
    if (await refreshCodex()) {
      $("codex-status").textContent = installing
        ? `Installed ${body.skill.name} for Codex.`
        : `Removed ${name} from Codex.`;
    }
  } catch (error) {
    await refreshCodex();
    showCodexError(connectionMessage(error));
  } finally {
    pendingCodex.delete(key);
    updateCodexButtons();
    if (fromList && !$("codex-list").contains(document.activeElement)) $("codex-title").focus();
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
    const source = skillSource(skill);
    star.dataset.starKey = source.key;
    star.setAttribute("aria-label", `Star ${skill.path}`);
    star.addEventListener("click", () => {
      setStar(skill, source, !starred.has(star.dataset.starKey));
    });
    title.append(star);
  }
  card.append(title);
  card.append(node("p", "skill-description", skill.description || "No description available."));
  card.append(node("code", "skill-path", skill.path));
  const actions = node("div", "skill-actions");
  const link = externalLink("View on GitHub ↗", skill.link, "skill-link");
  link.setAttribute("aria-label", `View ${skill.path} on GitHub (opens in a new tab)`);
  actions.append(link);
  if (codexEnabled) {
    const install = node("button", "codex-button");
    install.type = "button";
    const source = skillSource(skill);
    install.dataset.codexKey = source.key;
    install.dataset.codexPath = skill.path;
    install.addEventListener("click", () => {
      setCodex(skill, source, !codexSkills.has(install.dataset.codexKey));
    });
    actions.append(install);
  }
  card.append(actions);
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

function renderRepositories(result) {
  const failed = result.repositories.filter((repository) => repository.error);
  const withSkills = result.repositories.filter((repository) => !repository.error && repository.skill_count);
  const withoutSkills = result.repositories.filter((repository) => !repository.error && !repository.skill_count);
  const rows = document.createDocumentFragment();
  for (const repository of [...failed, ...withSkills]) {
    const row = node("li", repository.error ? "repository-result failed" : "repository-result");
    row.append(node("strong", "", repository.repository));
    const detail = node("p", "", repository.error || `${repository.skill_count} skill${repository.skill_count === 1 ? "" : "s"} · `);
    if (!repository.error) {
      const link = externalLink(`Commit ${repository.commit.slice(0, 7)} ↗`, `https://github.com/${repository.repository}/commit/${repository.commit}`);
      link.title = repository.commit;
      link.setAttribute("aria-label", `Scanned commit ${repository.commit} of ${repository.repository} on GitHub (opens in a new tab)`);
      detail.append(link);
    }
    row.append(detail);
    rows.append(row);
  }
  $("repository-results").replaceChildren(rows);
  const names = document.createDocumentFragment();
  withoutSkills.forEach((repository) => names.append(node("li", "", repository.commit ? repository.repository : `${repository.repository} (empty)`)));
  $("without-skills-list").replaceChildren(names);
  $("without-skills-summary").textContent = `${withoutSkills.length} repositor${withoutSkills.length === 1 ? "y" : "ies"} without skills`;
  $("repositories-without-skills").hidden = withoutSkills.length === 0;
  $("repositories-without-skills").open = false;
  $("incomplete-panel").hidden = failed.length === 0;
  $("incomplete-message").textContent = `${failed.length} of ${result.repositories.length} repositories could not be scanned, so their skills are missing from these results. Scan them individually or retry later.`;
  const forks = result.skipped_forks;
  $("result-commit").textContent = `${result.repositories.length} public repositor${result.repositories.length === 1 ? "y" : "ies"} scanned · ${withSkills.length} with skills · ${forks} fork${forks === 1 ? "" : "s"} skipped`;
  return failed.length;
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
        (!starredOnly || starred.has(skillSource(skill).key)) &&
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
  updateCodexButtons();
  $("result-count").textContent = `Showing ${visible.size} of ${inventory.skills.length} skills` + (groupedView
    ? ` in ${matchingGroups.length} of ${groups.length} groups (including standalone skills) · largest first`
    : " · sorted by path");
  $("no-results").hidden = visible.size > 0;
  $("clear-filters").hidden = inventory.skills.length === 0;
  if (!visible.size) {
    if (inventory.skills.length) {
      $("empty-title").textContent = "No skills match your filters";
      $("empty-description").textContent = "Try another search, or clear the filters to see all discovered skills.";
    } else if (inventory.organization !== undefined) {
      const scanned = inventory.repositories.some((repository) => !repository.error);
      $("empty-title").textContent = scanned ? "No SKILL.md files found" : "No repositories scanned";
      $("empty-description").textContent = scanned
        ? "The scan finished. There are no regular SKILL.md files on the default branches of the scanned repositories."
        : inventory.repositories.length
          ? "None of this organization’s repositories could be scanned. Review the failures above, then retry."
          : "This organization or user has no public repositories to scan, apart from any forks.";
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
  const organization = result.organization !== undefined;
  $("result-kind").textContent = organization ? "ORGANIZATION" : "REPOSITORY";
  $("result-repository").textContent = organization ? result.organization : result.repository;
  $("organization-repositories").hidden = !organization;
  $("skill-count").textContent = result.skills.length;
  const stats = result.aggregation.statistics;
  $("warning-count").textContent = stats.skills_with_warnings;
  $("similar-group-count").textContent = stats.similar_groups;
  $("grouped-skill-count").textContent = stats.grouped_skills;
  $("grouped-skill-share").textContent = `${percentage(stats.grouped_skills)} of all skills`;
  $("standalone-count").textContent = stats.standalone_skills;
  $("largest-group-count").textContent = stats.largest_group;
  $("result-commit").replaceChildren();
  let failed = 0;
  if (organization) {
    failed = renderRepositories(result);
  } else if (result.commit) {
    const link = externalLink(`Commit ${result.commit.slice(0, 7)} ↗`, `https://github.com/${result.repository}/commit/${result.commit}`);
    link.title = result.commit;
    link.setAttribute("aria-label", `Scanned commit ${result.commit} on GitHub (opens in a new tab)`);
    $("result-commit").append(link);
  } else {
    $("result-commit").textContent = "No commit to scan";
  }
  $("result-controls").hidden = result.skills.length === 0;
  $("results-status").textContent = failed
    ? `Incomplete · ${failed} repositor${failed === 1 ? "y" : "ies"} failed`
    : "Scan complete";
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
  const organization = organizationMode;
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
  $("results-status").textContent = organization ? "Scanning organization…" : "Scanning repository…";
  updateProgress({ message: "Connecting to GitHub…" });
  setBusy(true);
  try {
    const response = await fetch(organization ? "/api/scan-org" : "/api/scan", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Skill-Scanner": "1" },
      body: JSON.stringify(organization ? { organization: repository } : { repository }),
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
document.querySelectorAll("[data-organization]").forEach((button) => {
  button.addEventListener("click", () => {
    selectOrganization(button.dataset.organization);
  });
});
$("repository-mode").addEventListener("click", () => setMode(false));
$("organization-mode").addEventListener("click", () => setMode(true));
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
  refreshCodex();
});
refreshRecent();
refreshStarred();
refreshCodex();
