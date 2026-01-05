const statusEl = document.getElementById("status");
const gridEl = document.getElementById("vm-grid");
const refreshButton = document.getElementById("refresh");

const statusClasses = {
  running: "status-running",
  stopped: "status-stopped",
  unknown: "status-unknown",
};

function setStatus(message) {
  statusEl.textContent = message;
}

function renderVms(vms) {
  gridEl.innerHTML = "";
  vms.forEach((vm) => {
    const card = document.createElement("article");
    card.className = "vm-card";

    const header = document.createElement("header");
    const name = document.createElement("div");
    name.className = "vm-name";
    name.textContent = `${vm.name || "Unnamed"} (#${vm.vmid})`;

    const dot = document.createElement("span");
    dot.className = `status-dot ${statusClasses[vm.status] || "status-unknown"}`;

    header.appendChild(name);
    header.appendChild(dot);

    const tags = document.createElement("div");
    tags.className = "tags";
    if (vm.tags.length === 0) {
      const emptyTag = document.createElement("span");
      emptyTag.className = "tag";
      emptyTag.textContent = "no tags";
      tags.appendChild(emptyTag);
    } else {
      vm.tags.forEach((tag) => {
        const tagEl = document.createElement("span");
        tagEl.className = "tag";
        tagEl.textContent = tag;
        tags.appendChild(tagEl);
      });
    }

    const notes = document.createElement("div");
    notes.className = "notes";
    notes.textContent = vm.notes || "";

    const button = document.createElement("button");
    button.textContent = "Launch";
    button.addEventListener("click", () => launchVm(vm.vmid));

    card.appendChild(header);
    card.appendChild(tags);
    if (vm.notes) {
      card.appendChild(notes);
    }
    card.appendChild(button);

    gridEl.appendChild(card);
  });
}

async function loadVms() {
  setStatus("Loading VM inventory…");
  try {
    const response = await fetch("/api/vms");
    if (!response.ok) {
      throw new Error(`Failed to load VMs: ${response.status}`);
    }
    const vms = await response.json();
    renderVms(vms);
    setStatus(`Loaded ${vms.length} VMs.`);
  } catch (error) {
    console.error(error);
    setStatus("Unable to load VMs. Check server logs.");
  }
}

async function launchVm(vmid, action) {
  setStatus("Submitting launch request…");
  try {
    const payload = { vmid };
    if (action) {
      payload.action = action;
    }

    const response = await fetch("/api/launch", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      const err = await response.json().catch(() => ({}));
      throw new Error(err.error || `Launch failed: ${response.status}`);
    }

    const result = await response.json();
    setStatus(result.message);

    if (result.status === "needs_action") {
      const runningName = result.running_vm?.name || "Current VM";
      const actionChoice = prompt(
        `${runningName} is running. Choose action: ${result.allowed_actions.join(", ")}`
      );
      if (!actionChoice) {
        setStatus("Launch cancelled.");
        return;
      }
      await launchVm(vmid, actionChoice);
      return;
    }

    if (result.status === "started") {
      setStatus("Launch initiated. Waiting for completion…");
    }

    await loadVms();
  } catch (error) {
    console.error(error);
    setStatus(error.message);
  }
}

refreshButton.addEventListener("click", () => loadVms());

loadVms();
