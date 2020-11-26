#![no_std]
#![no_main]
#![feature(array_map, once_cell, min_const_generics)]

use core::convert::TryFrom;
use psx::dma::{Addr, Block, BlockLen, Control};
use psx::gpu::color::*;
use psx::gpu::framebuffer::*;
use psx::gpu::primitives::*;
use psx::gpu::texture::*;
use psx::gpu::vertex::*;
use psx::gpu::{DrawPort, GpuStat, Packet};
use psx::*;

psx::exe!();

fn main(mut io: IO) {
    let mut draw_port = io.take_draw_port().unwrap();
    let mut gpu_stat = io.take_gpu_stat().unwrap();
    let mut disp_port = io.take_disp_port().unwrap();
    let mut gpu_dma = io.take_gpu_dma().unwrap();
    let res = (320, 240);
    let mut fb = Framebuffer::new(
        &mut draw_port,
        &mut disp_port,
        Vertex::zero(),
        (0, 240),
        res,
        None,
    );
    let font = unzip!("../font.tim.zip");
    let (page, clut) = tim!(font).load(&mut draw_port, &mut gpu_dma);
    let mut printer = Printer {
        cursor: (0, 0).into(),
        size: (8, 16).into(),
        offset: (0, 0).into(),
        limits: res.into(),
        color: Color::white(),
        page,
        clut,
    };
    printer.print(b"hello world! This is a very long message. It goes on and on and on... In fact, it might even start overwriting itself if I keep writing such long run-on sentences. I should find a way to make sure that words aren't split across lines. But then again, this isn't the place to write my TODOs. That's TODO.md. It really doesn't get much use though :(. Let's see what other emojis I can make :) :/ :p xD :D ^^ :] :^) 0123456789 ~!@#$%^&*_+=; Still got a ways to go before the message starts overwriting itself. Another thing I should add is msg support to my panic handler. That would make debugging so much easier!", &mut draw_port, &mut gpu_stat);
    fb.swap(&mut draw_port, &mut disp_port);
    delay(10_000_000);
    printer.reset_cursor();
    printer.print(b"1 + 1 = ", &mut draw_port, &mut gpu_stat);
    printer.println(&[b'0' + 1 + 1], &mut draw_port, &mut gpu_stat);
    printer.println(
        b"That was fmt in a loose sense",
        &mut draw_port,
        &mut gpu_stat,
    );
    printer.print(
        b"Let's format something more complicated 0xdead << 16 | 0xbeef = ",
        &mut draw_port,
        &mut gpu_stat,
    );
    let expr = (0xdead << 16) | 0xbeef;
    printer.print_u32(expr, &mut draw_port, &mut gpu_stat);
    fb.swap(&mut draw_port, &mut disp_port);
}

struct Printer {
    cursor: Vertex,
    size: Vertex,
    offset: Vertex,
    limits: Vertex,
    color: Color,
    page: Page,
    clut: Option<Clut>,
}

// TODO: I want to make the mutable register references RefCell-like without
// incurring the runtime cost of RefCell.
impl Printer {
    fn reset_cursor(&mut self) {
        self.cursor = self.offset;
    }
    fn newline(&mut self) {
        let vshift = self.size.y();
        self.cursor.apply(|x, y| (0, y + vshift));
    }
    fn print_u32(&mut self, x: u32, draw_port: &mut DrawPort, gpu_stat: &mut GpuStat) {
        self.print(b"0x", draw_port, gpu_stat);
        let mut leading = true;
        for i in 0..8 {
            let nibble = (x >> ((7 - i) * 4)) & 0xF;
            if nibble != 0 {
                leading = false;
            };
            if !leading {
                let as_char = core::char::from_digit(nibble, 16).unwrap();
                let as_ascii = u32::try_from(as_char).unwrap() as u8;
                self.print(&[as_ascii], draw_port, gpu_stat);
            }
        }
    }
    fn println(&mut self, msg: &[u8], draw_port: &mut DrawPort, gpu_stat: &mut GpuStat) {
        self.print(msg, draw_port, gpu_stat);
        self.newline();
    }
    fn print(&mut self, msg: &[u8], draw_port: &mut DrawPort, gpu_stat: &mut GpuStat) {
        let w_as_u8 = self.size.x() as u8;
        let h_as_u8 = self.size.y() as u8;
        // This assumes that only one texture page is used
        let ascii_per_row = 128 / w_as_u8;
        for &ascii in msg {
            if ascii == b'\n' {
                self.newline();
            } else {
                let xoffset = ((ascii % ascii_per_row) * w_as_u8);
                let yoffset = ((ascii / ascii_per_row) * h_as_u8);
                let letter = textured_quad(
                    Vertex::offset_rect(self.cursor.shift(self.offset), self.size),
                    self.color,
                    [(0, 0), (0, h_as_u8), (w_as_u8, 0), (w_as_u8, h_as_u8)]
                        .map(|(x, y)| (x + xoffset, y + yoffset)),
                    self.page,
                    self.clut,
                );
                if self.cursor.x() + self.size.x() >= self.limits.x() {
                    let vshift = self.size.y();
                    self.cursor.apply(|x, y| (0, y + vshift));
                } else {
                    let hshift = self.size.x();
                    self.cursor.apply(|x, y| (x + hshift, y));
                }
                while !gpu_stat.ready() {}
                draw_port.send(&letter);
            }
        }
    }
}
