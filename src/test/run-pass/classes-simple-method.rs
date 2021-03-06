// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct cat {
    priv meows : uint,

    how_hungry : int,
}

pub impl cat {
  fn speak(&mut self) {}
}

fn cat(in_x : uint, in_y : int) -> cat {
    cat {
        meows: in_x,
        how_hungry: in_y
    }
}

pub fn main() {
  let mut nyan : cat = cat(52u, 99);
  let mut kitty = cat(1000u, 2);
  fail_unless!((nyan.how_hungry == 99));
  fail_unless!((kitty.how_hungry == 2));
  nyan.speak();
}
