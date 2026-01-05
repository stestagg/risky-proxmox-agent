const statusEl = document.getElementById("status");
const gridEl = document.getElementById("vm-grid");
const refreshButton = document.getElementById("refresh");
const actionDialog = document.getElementById("action-dialog");
const actionDialogTitle = document.getElementById("action-dialog-title");
const actionDialogButtons = document.getElementById("action-dialog-buttons");
const actionDialogCancel = document.getElementById("action-dialog-cancel");

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

    card.appendChild(header);
    card.appendChild(tags);
    if (vm.notes) {
      card.appendChild(notes);
    }
    if (vm.status !== "running") {
      const button = document.createElement("button");
      button.textContent = "Launch";
      button.addEventListener("click", () => launchVm(vm.vmid));
      card.appendChild(button);
    }

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
      const actionChoice = await promptForAction(
        runningName,
        result.allowed_actions
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

function promptForAction(runningName, actions) {
  return new Promise((resolve) => {
    actionDialogTitle.textContent = `${runningName} is running. Choose action:`;
    actionDialogButtons.innerHTML = "";

    const handleClose = () => {
      resolve(actionDialog.returnValue || null);
    };

    actionDialog.addEventListener("close", handleClose, { once: true });

    actions.forEach((action) => {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = action;
      button.addEventListener("click", () => actionDialog.close(action));
      actionDialogButtons.appendChild(button);
    });

    actionDialogCancel.onclick = () => actionDialog.close("");
    actionDialog.showModal();
  });
}
