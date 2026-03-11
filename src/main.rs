use std::{collections::HashMap, fs, thread::sleep, time::Duration};

use system_monitor::{hashmap, parser::{Parser, ProcessParser, parser::TraitProcessParser}, state::SystemState, util::ParseError};

fn main() -> Result<(), ParseError>{
    let mut system_state = SystemState::new();
    let process_parser = ProcessParser::new();
    let parser = Parser::new(process_parser);

    for entry in fs::read_dir("/proc").unwrap() {
        let process_path = entry.unwrap().path();
        if process_path.is_dir() {
            let process_path_string = process_path.display().to_string();
            let _process = parser.parse_process(&mut system_state, &process_path_string);
            //println!("{:#?}", process);
        }
    }

    let sys0 = parser.get_status_info()?;
    let num_cores = sys0.num_cores as f64;
    let total0 = sys0.total_cpu;

    let mut prev: HashMap<u32, u64> = hashmap!();
    for p in system_state.processes.values() {
        if let Ok((utime, stime)) = parser.process_parser.get_stat_info(p.pid) {
            prev.insert(p.pid, utime + stime);
        }
    }

    sleep(Duration::from_millis(2000));

    let total1 = parser.get_status_info()?.total_cpu;
    let d_total = total1.saturating_sub(total0);
    if d_total == 0 {
        return Ok(());
    }

    for p in system_state.processes.values() {
        if let (Some(prev_j), Ok((utime, stime))) =
        (prev.get(&p.pid), parser.process_parser.get_stat_info(p.pid))
        {
            let curr: u64 = utime + stime;
            let d_proc = curr.saturating_sub(*prev_j);

            let pct_norm = 100.0 * (d_proc as f64) / (d_total as f64);
            let pct_top = pct_norm * num_cores;

            println!(
                "pid={} name={} cpu_norm={:.2}% cpu_top={:.2}%",
                p.pid, p.name, pct_norm, pct_top
            );
        }
    }
    println!("{:#?}", system_state);
    println!("{:#?}", parser.get_status_info());

    Ok(())
}
