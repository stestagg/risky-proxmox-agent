const statusEl = document.getElementById("status");
const gridEl = document.getElementById("vm-grid");
const refreshButton = document.getElementById("refresh");
const actionDialog = document.getElementById("action-dialog");
const actionDialogTitle = document.getElementById("action-dialog-title");
const actionDialogButtons = document.getElementById("action-dialog-buttons");
const actionDialogCancel = document.getElementById("action-dialog-cancel");
const forkDialog = document.getElementById("fork-dialog");
const forkDialogForm = document.getElementById("fork-dialog-form");
const forkDialogTitle = document.getElementById("fork-dialog-title");
const forkNameInput = document.getElementById("fork-name-input");
const forkDialogCancel = document.getElementById("fork-dialog-cancel");
const shutdownButton = document.getElementById("shutdown-host");
const shutdownDialog = document.getElementById("shutdown-dialog");
const shutdownDialogConfirm = document.getElementById("shutdown-dialog-confirm");
const shutdownDialogCancel = document.getElementById("shutdown-dialog-cancel");

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

    const actions = document.createElement("div");
    actions.className = "actions";

    if (vm.status !== "running") {
      const launchButton = document.createElement("button");
      launchButton.textContent = "Launch";
      launchButton.addEventListener("click", () => launchVm(vm.vmid));
      actions.appendChild(launchButton);
    }

    const forkButton = document.createElement("button");
    forkButton.className = "secondary";
    forkButton.textContent = "Fork";
    forkButton.addEventListener("click", () => forkVm(vm));
    actions.appendChild(forkButton);

    card.appendChild(actions);

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

async function forkVm(vm) {
  const forkName = await promptForForkName(vm);
  if (!forkName) {
    setStatus("Fork cancelled.");
    return;
  }

  setStatus("Creating fork…");
  try {
    const response = await fetch("/api/fork", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ vmid: vm.vmid, name: forkName }),
    });

    if (!response.ok) {
      const err = await response.json().catch(() => ({}));
      throw new Error(err.error || `Fork failed: ${response.status}`);
    }

    const result = await response.json();
    setStatus(result.message || "Fork created.");
    await loadVms();
  } catch (error) {
    console.error(error);
    setStatus(error.message);
  }
}

refreshButton.addEventListener("click", () => loadVms());
shutdownButton.addEventListener("click", () => requestHostShutdown());

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

function promptForForkName(vm) {
  return new Promise((resolve) => {
    const baseName = vm.name || `vm-${vm.vmid}`;
    forkDialogTitle.textContent = `Fork ${baseName} (#${vm.vmid})`;
    forkNameInput.value = `${baseName}-fork`;

    const handleClose = () => {
      const value = forkDialog.returnValue || "";
      resolve(value.trim() || null);
    };

    forkDialog.addEventListener("close", handleClose, { once: true });

    forkDialogCancel.onclick = () => forkDialog.close("");

    forkDialogForm.addEventListener(
      "submit",
      (event) => {
        event.preventDefault();
        const value = forkNameInput.value.trim();
        if (!value) {
          forkNameInput.focus();
          return;
        }
        forkDialog.close(value);
      },
      { once: true }
    );

    forkDialog.showModal();
    forkNameInput.focus();
    forkNameInput.select();
  });
}

async function requestHostShutdown(action) {
  if (!action) {
    const confirmed = await confirmHostShutdown();
    if (!confirmed) {
      setStatus("Host shutdown cancelled.");
      return;
    }
  }

  setStatus("Submitting host shutdown request…");
  try {
    const payload = {};
    if (action) {
      payload.action = action;
    }

    const response = await fetch("/api/host-shutdown", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      const err = await response.json().catch(() => ({}));
      throw new Error(err.error || `Shutdown failed: ${response.status}`);
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
        setStatus("Host shutdown cancelled.");
        return;
      }
      await requestHostShutdown(actionChoice);
      return;
    }
  } catch (error) {
    console.error(error);
    setStatus(error.message);
  }
}

function confirmHostShutdown() {
  return new Promise((resolve) => {
    const handleClose = () => {
      resolve(shutdownDialog.returnValue === "confirm");
    };

    shutdownDialog.addEventListener("close", handleClose, { once: true });

    shutdownDialogConfirm.onclick = () => shutdownDialog.close("confirm");
    shutdownDialogCancel.onclick = () => shutdownDialog.close("");

    shutdownDialog.showModal();
  });
}
