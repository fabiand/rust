// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait MyNum {
    static fn from_int(int) -> Self;
}

pub trait NumExt: MyNum { }

struct S { v: int }

impl MyNum for S {
    static fn from_int(i: int) -> S {
        S {
            v: i
        }
    }
}

impl NumExt for S { }

fn greater_than_one<T:NumExt>() -> T { MyNum::from_int(1) }

pub fn main() {
    let v: S = greater_than_one();
    fail_unless!(v.v == 1);
}
