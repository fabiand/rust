// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

const a: [u8 * 3] = ['h' as u8, 'i' as u8, 0 as u8];
const c: &'static [u8 * 3] = &a;
const b: *u8 = c as *u8;

fn main() {
    let foo = &a as *u8;
    fail_unless!(unsafe { str::raw::from_bytes(a) } == ~"hi\x00");
    fail_unless!(unsafe { str::raw::from_buf(foo) } == ~"hi");
    fail_unless!(unsafe { str::raw::from_buf(b) } == ~"hi");
    fail_unless!(unsafe { *b == a[0] });
    fail_unless!(unsafe { *(&c[0] as *u8) == a[0] });
}
