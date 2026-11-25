// A sample of every definition kind: the definitions below are listed in `kinds.expected`.

mod inline {
    pub fn in_module() {}
}
mod outline;

extern "C" {
    fn from_c(x: i32) -> i32;
    static C_STATE: i32;
}

fn outer() {
    fn nested() {}
}

struct Plain;
enum Choice {
    A,
}
union Bits {
    a: u32,
    b: f32,
}

trait Speak {
    type Out;
    const LOUD: bool;
    fn bare(&self);
    fn with_body(&self) {}
}

impl Plain {
    type Alias = u8;
    const LIMIT: u8 = 1;
    fn method(&self) {}
}

type Pair = (i32, i32);
const MAX: i32 = 9;
static GLOBAL: i32 = 0;

macro_rules! twice {
    ($e:expr) => {
        $e * 2
    };
}

#[cfg(unix)]
fn variant() {}
#[cfg(windows)]
fn variant() {}
