//! Calculator for the Windows 95, 98 and 2000 looks: each look's own Calculator, as the
//! Reference PCs showed it.
//!
//! - `number`: the decimal numbers of the 98 and 2000 Calculators (32 digits shown).
//! - `engine`: what each Calculator computes and shows, key by key.
//! - `form`: the main window (the Standard and Scientific views) as a control kit form.
//! - `stats`: the Statistics Box of the Scientific view.
//! - `art`: its picture. `dialogs`: About Calculator. `settings`: calculator.txt.
//! - `help`: each look's Help book.

pub mod art;
pub mod dialogs;
pub mod engine;
pub mod form;
pub mod help;
pub mod number;
pub mod settings;
pub mod stats;
mod captions95;
