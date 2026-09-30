"use strict";

const $ = (id) => document.getElementById(id);
let inventory = null;
let busy = false;
let groupedView = true;

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
  $("scan-button-text").textContent = value ? "Scanning…" : "Scan repository";
  $("results-section").setAttribute("aria-busy", String(value));
  $("progress-panel").hidden = !value;
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
  card.append(node(heading, "", skill.name));
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
  const groups = inventory.aggregation.groups;
  const visible = new Set();
  const matchingGroups = [];
  for (const group of groups) {
    if (similarOnly && group.skill_indices.length < 2) continue;
    const indices = group.skill_indices.filter((index) => {
      const skill = inventory.skills[index];
      return (!warningsOnly || skill.warnings.length > 0) &&
        [skill.name, skill.description, skill.path].some((text) => text.toLocaleLowerCase().includes(query));
    });
    if (indices.length) {
      matchingGroups.push({ group, indices });
      indices.forEach((index) => visible.add(index));
    }
  }
  const cards = document.createDocumentFragment();
  if (groupedView) {
    matchingGroups.forEach(({ group, indices }) => cards.append(groupCard(group, indices, Boolean(query || warningsOnly))));
  } else {
    inventory.skills.forEach((skill, index) => { if (visible.has(index)) cards.append(skillCard(skill)); });
  }
  $("skill-list").classList.toggle("grouped", groupedView);
  $("skill-list").replaceChildren(cards);
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
  $("scan-results").hidden = true;
  $("initial-state").hidden = true;
  $("search").value = "";
  $("warnings-only").checked = false;
  $("similar-only").checked = false;
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
  }
});

document.querySelectorAll("[data-repository]").forEach((button) => {
  button.addEventListener("click", () => {
    $("repository").value = button.dataset.repository;
    $("repository").focus();
  });
});
$("search").addEventListener("input", renderSkills);
$("warnings-only").addEventListener("change", renderSkills);
$("similar-only").addEventListener("change", renderSkills);
$("grouped-view").addEventListener("click", () => setView(true));
$("all-view").addEventListener("click", () => setView(false));
$("clear-filters").addEventListener("click", () => {
  $("search").value = "";
  $("warnings-only").checked = false;
  $("similar-only").checked = false;
  renderSkills();
  $("search").focus();
});
