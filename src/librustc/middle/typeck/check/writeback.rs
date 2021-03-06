// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Type resolution: the phase that finds all the types in the AST with
// unresolved type variables and replaces "ty_var" types with their
// substitutions.

use core::prelude::*;

use middle::pat_util;
use middle::ty::arg;
use middle::ty;
use middle::typeck::check::{FnCtxt, SelfInfo};
use middle::typeck::infer::{force_all, resolve_all, resolve_region};
use middle::typeck::infer::{resolve_type};
use middle::typeck::infer;
use middle::typeck::method_map_entry;
use middle::typeck::{vtable_param, write_substs_to_tcx};
use middle::typeck::{write_ty_to_tcx};
use util::ppaux;

use core::result::{Result, Ok, Err};
use core::vec;
use syntax::ast;
use syntax::codemap::span;
use syntax::print::pprust::pat_to_str;
use syntax::visit;

fn resolve_type_vars_in_type(fcx: @mut FnCtxt, sp: span, typ: ty::t)
                          -> Option<ty::t> {
    if !ty::type_needs_infer(typ) { return Some(typ); }
    match resolve_type(fcx.infcx(), typ, resolve_all | force_all) {
        Ok(new_type) => return Some(new_type),
        Err(e) => {
            if !fcx.ccx.tcx.sess.has_errors() {
                fcx.ccx.tcx.sess.span_err(
                    sp,
                    fmt!("cannot determine a type \
                          for this expression: %s",
                         infer::fixup_err_to_str(e)))
            }
            return None;
        }
    }
}

fn resolve_method_map_entry(fcx: @mut FnCtxt, sp: span, id: ast::node_id) {
    // Resolve any method map entry
    match fcx.ccx.method_map.find(&id) {
        None => {}
        Some(ref mme) => {
            for resolve_type_vars_in_type(fcx, sp, mme.self_arg.ty).each |t| {
                let method_map = fcx.ccx.method_map;
                method_map.insert(id,
                                  method_map_entry {
                                    self_arg: arg {
                                        mode: mme.self_arg.mode,
                                        ty: *t
                                    },
                                    .. *mme
                                  });
            }
        }
    }
}

fn resolve_type_vars_for_node(wbcx: @mut WbCtxt, sp: span, id: ast::node_id)
                           -> Option<ty::t> {
    let fcx = wbcx.fcx, tcx = fcx.ccx.tcx;

    // Resolve any borrowings for the node with id `id`
    match fcx.inh.adjustments.find(&id) {
        None => (),

        Some(@ty::AutoAddEnv(r, s)) => {
            match resolve_region(fcx.infcx(), r, resolve_all | force_all) {
                Err(e) => {
                    // This should not, I think, happen:
                    fcx.ccx.tcx.sess.span_err(
                        sp, fmt!("cannot resolve bound for closure: %s",
                                 infer::fixup_err_to_str(e)));
                }
                Ok(r1) => {
                    let resolved_adj = @ty::AutoAddEnv(r1, s);
                    debug!("Adjustments for node %d: %?", id, resolved_adj);
                    fcx.tcx().adjustments.insert(id, resolved_adj);
                }
            }
        }

        Some(@ty::AutoDerefRef(adj)) => {
            let resolved_autoref = match adj.autoref {
                Some(ref autoref) => {
                    match resolve_region(fcx.infcx(), autoref.region,
                                         resolve_all | force_all) {
                        Err(e) => {
                            // This should not, I think, happen.
                            fcx.ccx.tcx.sess.span_err(
                                sp, fmt!("cannot resolve scope of borrow: %s",
                                         infer::fixup_err_to_str(e)));
                            Some(*autoref)
                        }
                        Ok(r) => {
                            Some(ty::AutoRef {region: r, ..*autoref})
                        }
                    }
                }
                None => None
            };

            let resolved_adj = @ty::AutoDerefRef(ty::AutoDerefRef {
                autoderefs: adj.autoderefs,
                autoref: resolved_autoref,
            });
            debug!("Adjustments for node %d: %?", id, resolved_adj);
            fcx.tcx().adjustments.insert(id, resolved_adj);
        }
    }

    // Resolve the type of the node with id `id`
    let n_ty = fcx.node_ty(id);
    match resolve_type_vars_in_type(fcx, sp, n_ty) {
      None => {
        wbcx.success = false;
        return None;
      }

      Some(t) => {
        debug!("resolve_type_vars_for_node(id=%d, n_ty=%s, t=%s)",
               id, ppaux::ty_to_str(tcx, n_ty), ppaux::ty_to_str(tcx, t));
        write_ty_to_tcx(tcx, id, t);
        match fcx.opt_node_ty_substs(id) {
          Some(ref substs) => {
            let mut new_tps = ~[];
            for (*substs).tps.each |subst| {
                match resolve_type_vars_in_type(fcx, sp, *subst) {
                  Some(t) => new_tps.push(t),
                  None => { wbcx.success = false; return None; }
                }
            }
            write_substs_to_tcx(tcx, id, new_tps);
          }
          None => ()
        }
        return Some(t);
      }
    }
}

fn maybe_resolve_type_vars_for_node(wbcx: @mut WbCtxt,
                                    sp: span,
                                    id: ast::node_id)
                                 -> Option<ty::t> {
    if wbcx.fcx.inh.node_types.contains_key(&id) {
        resolve_type_vars_for_node(wbcx, sp, id)
    } else {
        None
    }
}

struct WbCtxt {
    fcx: @mut FnCtxt,

    // As soon as we hit an error we have to stop resolving
    // the entire function.
    success: bool,
}

type wb_vt = visit::vt<@mut WbCtxt>;

fn visit_stmt(s: @ast::stmt, &&wbcx: @mut WbCtxt, v: wb_vt) {
    if !wbcx.success { return; }
    resolve_type_vars_for_node(wbcx, s.span, ty::stmt_node_id(s));
    visit::visit_stmt(s, wbcx, v);
}
fn visit_expr(e: @ast::expr, &&wbcx: @mut WbCtxt, v: wb_vt) {
    if !wbcx.success { return; }
    resolve_type_vars_for_node(wbcx, e.span, e.id);
    resolve_method_map_entry(wbcx.fcx, e.span, e.id);
    resolve_method_map_entry(wbcx.fcx, e.span, e.callee_id);
    match e.node {
      ast::expr_fn_block(ref decl, _) => {
          for vec::each(decl.inputs) |input| {
              let r_ty = resolve_type_vars_for_node(wbcx, e.span, input.id);

              // Just in case we never constrained the mode to anything,
              // constrain it to the default for the type in question.
              match (r_ty, input.mode) {
                  (Some(t), ast::infer(_)) => {
                      let tcx = wbcx.fcx.ccx.tcx;
                      let m_def = ty::default_arg_mode_for_ty(tcx, t);
                      ty::set_default_mode(tcx, input.mode, m_def);
                  }
                  _ => ()
              }
          }
      }

      ast::expr_binary(*) | ast::expr_unary(*) | ast::expr_assign_op(*)
        | ast::expr_index(*) => {
        maybe_resolve_type_vars_for_node(wbcx, e.span, e.callee_id);
      }

      ast::expr_method_call(*) => {
        // We must always have written in a callee ID type for these.
        resolve_type_vars_for_node(wbcx, e.span, e.callee_id);
      }

      _ => ()
    }
    visit::visit_expr(e, wbcx, v);
}
fn visit_block(b: &ast::blk, &&wbcx: @mut WbCtxt, v: wb_vt) {
    if !wbcx.success { return; }
    resolve_type_vars_for_node(wbcx, b.span, b.node.id);
    visit::visit_block(b, wbcx, v);
}
fn visit_pat(p: @ast::pat, &&wbcx: @mut WbCtxt, v: wb_vt) {
    if !wbcx.success { return; }
    resolve_type_vars_for_node(wbcx, p.span, p.id);
    debug!("Type for pattern binding %s (id %d) resolved to %s",
           pat_to_str(p, wbcx.fcx.ccx.tcx.sess.intr()), p.id,
           wbcx.fcx.infcx().ty_to_str(
               ty::node_id_to_type(wbcx.fcx.ccx.tcx,
                                   p.id)));
    visit::visit_pat(p, wbcx, v);
}
fn visit_local(l: @ast::local, &&wbcx: @mut WbCtxt, v: wb_vt) {
    if !wbcx.success { return; }
    let var_ty = wbcx.fcx.local_ty(l.span, l.node.id);
    match resolve_type(wbcx.fcx.infcx(), var_ty, resolve_all | force_all) {
        Ok(lty) => {
            debug!("Type for local %s (id %d) resolved to %s",
                   pat_to_str(l.node.pat, wbcx.fcx.tcx().sess.intr()),
                   l.node.id,
                   wbcx.fcx.infcx().ty_to_str(lty));
            write_ty_to_tcx(wbcx.fcx.ccx.tcx, l.node.id, lty);
        }
        Err(e) => {
            wbcx.fcx.ccx.tcx.sess.span_err(
                l.span,
                fmt!("cannot determine a type \
                      for this local variable: %s",
                     infer::fixup_err_to_str(e)));
            wbcx.success = false;
        }
    }
    visit::visit_local(l, wbcx, v);
}
fn visit_item(_item: @ast::item, &&_wbcx: @mut WbCtxt, _v: wb_vt) {
    // Ignore items
}

fn mk_visitor() -> visit::vt<@mut WbCtxt> {
    visit::mk_vt(@visit::Visitor {visit_item: visit_item,
                                  visit_stmt: visit_stmt,
                                  visit_expr: visit_expr,
                                  visit_block: visit_block,
                                  visit_pat: visit_pat,
                                  visit_local: visit_local,
                                  .. *visit::default_visitor()})
}

pub fn resolve_type_vars_in_expr(fcx: @mut FnCtxt, e: @ast::expr) -> bool {
    let wbcx = @mut WbCtxt { fcx: fcx, success: true };
    let visit = mk_visitor();
    (visit.visit_expr)(e, wbcx, visit);
    return wbcx.success;
}

pub fn resolve_type_vars_in_fn(fcx: @mut FnCtxt,
                               decl: &ast::fn_decl,
                               blk: &ast::blk,
                               self_info: Option<SelfInfo>) -> bool {
    let wbcx = @mut WbCtxt { fcx: fcx, success: true };
    let visit = mk_visitor();
    (visit.visit_block)(blk, wbcx, visit);
    for self_info.each |self_info| {
        resolve_type_vars_for_node(wbcx,
                                   self_info.span,
                                   self_info.self_id);
    }
    for decl.inputs.each |arg| {
        do pat_util::pat_bindings(fcx.tcx().def_map, arg.pat)
                |_bm, pat_id, span, _path| {
            resolve_type_vars_for_node(wbcx, span, pat_id);
        }
        // Privacy needs the type for the whole pattern, not just each binding
        if !pat_util::pat_is_binding(fcx.tcx().def_map, arg.pat) {
            resolve_type_vars_for_node(wbcx, arg.pat.span, arg.pat.id);
        }
    }
    return wbcx.success;
}
