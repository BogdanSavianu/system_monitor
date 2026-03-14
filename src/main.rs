use std::{thread::sleep, time::Duration};

use system_monitor::{monitor::Monitor, util::ParseError};

fn main() -> Result<(), ParseError>{
    let mut monitor = Monitor::new();

    // t0
    monitor.initialize_sampling()?;

    sleep(Duration::from_millis(2000));

    // t1
    let proc_jiffies = monitor.sample_cpu_usage()?;
    let system_state = monitor.state();

    for (pid, usage) in proc_jiffies.iter() {
        if let Some(proc) = system_state.processes.get(pid) {
            println!(
                "pid={} name={} cpu_usage={:.2}%",
                proc.pid, proc.name, usage
            );
        }
    }

    //println!("{:#?}", system_state);
    //println!("{:#?}", parser.get_status_info());

    Ok(())
}
