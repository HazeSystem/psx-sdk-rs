use psx::dma;
use psx::framebuffer::Framebuffer;
use psx::general::*;
use psx::lazy_global;
use psx::printer::{Printer, MIN_SIZE};

#[allow(dead_code)]
const RUN_CONST_TESTS: () = {
    use super::CONST_TESTS;

    let mut i = 0;
    while i < CONST_TESTS.len() {
        let _ = CONST_TESTS[i] as usize - 1;
        i += 1;
    }
};

lazy_global!(let PRINTER: Printer<MIN_SIZE> = {
    let mut printer = Printer::new(0, 0, (320, 240), None);
    printer.load_font(&mut dma::gpu::CHCR::new());
    printer
});

macro_rules! print {
    ($msg:expr) => {
        $crate::framework::PRINTER
            .get()
            .print($msg, [], &mut unsafe { dma::gpu::CHCR::new() });
    };
    ($msg:expr, $arg0:expr) => {
        $crate::framework::PRINTER
            .get()
            .print($msg, [$arg0], &mut unsafe { dma::gpu::CHCR::new() });
    };
}

#[no_mangle]
fn main(mut gpu_dma: dma::gpu::CHCR) -> ! {
    reset_graphics(&mut gpu_dma);
    let _fb = Framebuffer::new((0, 0), (0, 240), (320, 240), None, &mut gpu_dma);
    enable_display();

    print!(b"Running tests...\n");
    for &t in &super::TESTS {
        run_test(t);
    }
    loop {}
}

fn run_test(f: fn() -> bool) {
    let msg = if f() {
        b"Passed test {}\n"
    } else {
        b"Failed test {}\n"
    };
    static mut TEST_NUM: u32 = 0;
    unsafe {
        TEST_NUM += 1;
    }
    print!(msg, unsafe { TEST_NUM });
}
