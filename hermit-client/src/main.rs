#[cfg(target_os = "hermit")]
use hermit as _;

mod api;

const BASE_URL: &str = "http://10.0.2.2:8080";

fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    println!("Fetching VM list from {}...", BASE_URL);

    use std::io::{Read, Write};
    use std::net::TcpStream;

    let mut s = TcpStream::connect("10.0.2.2:8080").expect("Failed to connect to server");
    s.write_all(b"GET /api/vms HTTP/1.1\r\nHost: 10.0.2.2\r\nConnection: close\r\n\r\n")
        .expect("Failed to send request");  

    let mut buf = String::new();
    s.read_to_string(&mut buf)        .expect("Failed to read response");
    println!("{buf}");

    // match api::fetch_vms(BASE_URL) {
    //     Ok(vms) => {
    //         if vms.is_empty() {
    //             println!("No VMs found.");
    //         } else {
    //             println!("{} VM(s):", vms.len());
    //             for vm in &vms {
    //                 println!(
    //                     "  [{vmid}] {name}  status={status}  tags=[{tags}]{notes}",
    //                     vmid = vm.vmid,
    //                     name = vm.name,
    //                     status = vm.status,
    //                     tags = vm.tags.join(", "),
    //                     notes = vm.notes.as_deref()
    //                         .map(|n| format!("  notes={n}"))
    //                         .unwrap_or_default(),
    //                 );
    //             }
    //         }
    //     }
    //     Err(e) => {
    //         eprintln!("Error fetching VMs: {e}");
    //         std::process::exit(1);
    //     }
    // }
}
