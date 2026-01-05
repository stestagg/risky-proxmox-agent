# risky-proxmox-agent
A launcher api for managing mutually-exclusive VMs

risky-proxmox-agent is a rust based micro web UI that provides a simple VM launcher interface onto a proxmox server.

It's 'risky' becuse it's designed for fully trusted private environments where no authentication is required.  If the user can access the server, they can do anything the server provides.

The UI element is:
A single page with an attractive grid of all VMs available on the proxmox server (using the API, the token and pve host in a .env file, pve host may be localhost, self-signed ssl should be accepted).
Each grid item has the name, list of tags, and any notes shown nicely, Colour can be used to indicate a Running VM. 

Clicking a VM causes this sequence to happen (apart from the initial confirmation) server-side (the client may be on a VM that will be shutdown so it has to be fully server-side and independent of any HTTP request)
 - If there is a running VM, if it has the 'easy-kill' tag, then stop the VM immediately (terminate), otherwise prompt the user [Current VM: Shutdown gracefully, Hibernate, Terminate, Cancel].
 - Perform the seleted action on the running vm 
 - Wait for the vm to fully stop (if one is running) to ensure that the resources are freed
 - Launch the selected new VM

Only one launch action can be running at any time.  If a users selects 'Shutdown gracefully', but then repeats the request with 'Terminate', then the Terminate should be applied to the current VM, and the existing process continue, otherwise an error should be returned/displayed, indicating that a VM launch is currently happening.

# Code Structure
A top-level binary crate that runs the webserver using .env files for pve values, and command line arguments (clap?) for bind address/port.
HTML/JS files are embedded in the binary (using askama?) 
An example systemd unit for running the service on startup on a proxmox server
INSTALLING and RUNNING markdown files showing how to install/run the service.
