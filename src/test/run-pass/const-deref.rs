// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

const C: &'static int = &1000;
const D: int = *C;
struct S(&'static int);
const E: &'static S = &S(C);
const F: int = ***E;

pub fn main() {
    fail_unless!(D == 1000);
    fail_unless!(F == 1000);
}
