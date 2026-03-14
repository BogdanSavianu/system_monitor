use std::{thread::sleep, time::Duration};

use system_monitor::{parser::{Parser, ProcessParser}, state::SystemState, util::ParseError};

fn main() -> Result<(), ParseError>{
    let mut system_state = SystemState::new();
    let process_parser = ProcessParser::new();
    let parser = Parser::new(process_parser);

    // t0
    let total0 = parser.initialize_cpu_sampling(&mut system_state)?;

    sleep(Duration::from_millis(2000));

    // t1
    parser.refresh_process_snapshot(&mut system_state);
    let new_jiffies = parser.get_process_jiffies(&system_state);

    let total1 = parser.get_status_info()?.total_cpu;

    let proc_jiffies = system_state.calculate_cpu_usage(new_jiffies, total0, total1);

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
