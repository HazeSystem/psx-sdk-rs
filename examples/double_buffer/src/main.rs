#![no_std]
#![no_main]

use framebuffer::constants::*;
use framebuffer::{draw_sync, enable_vblank, vsync, Framebuffer, Plane3, Result, V2, V3};
use psx::constants::*;
use psx::dma;
use psx::gpu::Color;
use psx::gpu::OrderingTable;
use psx::hw::cop0;
use psx::hw::Register;
use psx::println;
use psx::sys::gamepad::{Gamepad, BUFFER_SIZE};

psx::sys_heap! {
    unsafe {
        psx::data_cache()
    }
}

const BUF0: V2 = ZERO2;
const BUF1: V2 = V2(0, 240);
const RES: V2 = V2(320, 240);

fn basic(v: V3) -> V2 {
    V2(v.0, v.1)
}

fn sum_z((_, plane): &(Color, Plane3)) -> i16 {
    let mut res = 0;
    for V3(_, _, z) in *plane {
        res += z;
    }
    -res
}

fn poll_controller(pad: &Gamepad, pos: &mut V3, theta: &mut f32, phi: &mut f32) {
    let pos_y = Y;
    let pos_x = X;
    let pad = pad.poll();
    if pad.pressed(UP) {
        *pos -= pos_y;
    } else if pad.pressed(DOWN) {
        *pos += pos_y;
    }

    if pad.pressed(LEFT) {
        *pos -= pos_x;
    } else if pad.pressed(RIGHT) {
        *pos += pos_x;
    }

    if pad.pressed(CROSS) {
        *theta -= 0.1;
    } else if pad.pressed(TRIANGLE) {
        *theta += 0.1;
    }

    if pad.pressed(CIRCLE) {
        *phi -= 0.1;
    } else if pad.pressed(SQUARE) {
        *phi += 0.1;
    }
}

#[no_mangle]
fn main() -> Result<()> {
    let mut fb = Framebuffer::new(BUF0, BUF1, RES)?;
    let mut gpu_dma = dma::GPU::new();
    let mut buf0 = [0; BUFFER_SIZE];
    let mut buf1 = [0; BUFFER_SIZE];
    let pad = Gamepad::new(&mut buf0, &mut buf1)?;

    let scale = 50;
    let center = (X + Y + Z) * 50 / 2;
    let unit = [
        (BLUE, XY),
        (GREEN, YZ),
        (RED, XZ),
        (YELLOW, XY + Z),
        (CYAN, YZ + X),
        (VIOLET, XZ + Y),
    ]
    .map(|(c, p)| (c, (p * scale) - center));
    let mut pos = V3(120, 80, 50);
    let mut theta = 0.0;
    let mut phi = 0.0;

    let mut quads = OrderingTable::new([PolyF4::new(); 12])?;
    quads.link(0..6);
    quads.link(6..12);

    let mut display_a = true;
    loop {
        let (list_a, list_b) = quads.list.split_at_mut(6);
        let (display_list, draw_list) = if display_a {
            (list_a, list_b)
        } else {
            (list_b, list_a)
        };
        gpu_dma.send_list_and(display_list, || {
            let mut cube = unit.map(|(c, p)| (c, p.Rx(theta, ZERO).Ry(phi, ZERO) + pos));
            cube.sort_by_key(sum_z);
            for face in 0..6 {
                let (color, plane) = cube[face];
                draw_list[face].payload.set_vertices(plane.project(basic).into()).set_color(color);
            }
        })?;
        display_a = !display_a;
        poll_controller(&pad, &mut pos, &mut theta, &mut phi);
        draw_sync();
        vsync();
        fb.swap(Some(&mut gpu_dma))?;
    }
}
