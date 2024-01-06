mod xous;

use std::io::Read;
use xous::Machine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(target_program) = args.get(1) else {
        return Err(format!("Usage: {} <target-program>", args.first().expect("jurubas")).into());
    };

    let mut std_tests = Vec::new();
    std::fs::File::open(target_program)?.read_to_end(&mut std_tests)?;

    let mut xous = Machine::new(&std_tests)?;

    xous.run()?;

    Ok(())
}
