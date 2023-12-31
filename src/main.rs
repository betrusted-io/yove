mod xous;

use std::io::Read;
use xous::Machine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut std_tests = Vec::new();
    std::fs::File::open("std-tests")
        .expect("couldn't open std-tests")
        .read_to_end(&mut std_tests)
        .expect("couldn't read std-tests");

    let mut xous = Machine::new(&std_tests)?;

    xous.run()?;

    Ok(())
}
