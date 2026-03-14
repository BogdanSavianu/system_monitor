use std::{thread::sleep, time::Duration};

use system_monitor::{monitor::Monitor, util::ParseError};

fn main() -> Result<(), ParseError>{
    let mut monitor = Monitor::new();

    // t0
    monitor.initialize_sampling()?;

    sleep(Duration::from_millis(2000));

    // t1
    let cpu_samples = monitor.sample_cpu_usage()?;

    for sample in cpu_samples.iter() {
        println!(
            "pid={} name={} cpu_norm={:.2}% cpu_top={:.2}%",
            sample.pid, sample.name, sample.cpu_norm, sample.cpu_top
        );
    }

    //println!("{:#?}", system_state);
    //println!("{:#?}", parser.get_status_info());

    Ok(())
}
