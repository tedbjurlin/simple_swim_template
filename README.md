## Pluggable Interrupt Template

This project is a template that one can clone in order to set up
a [Pluggable Interrupt OS](https://crates.io/crates/pluggable_interrupt_os).

It demonstrates a simple interactive program that uses both keyboard and timer interrupts. 
When the user types a viewable key, it is added to a string in the middle of the screen. 
When the user types an arrow key, the string begins moving in the indicated direction.

The program logic is largely in `lib.rs`, in the `LetterMover` struct. The code in 
`main.rs` creates a `Mutex`-protected `LetterMover` object. The keyboard and timer handlers
invoke the appropriate methods on the unlocked `LetterMover` object.

This design pattern is highly recommended. Keep `main.rs` minimal, and encapsulate the 
application logic a struct that is defined in `lib.rs`. For your own applications, you can
use `LetterMover` as a starting point without modifying `main.rs` very much.

Prior to building this example, be sure to install the following:
* [Qemu](https://www.qemu.org/)
* Nightly Rust:
  * `rustup override set nightly`
* `llvm-tools-preview`:
  * `rustup component add llvm-tools-preview`
* The [bootimage](https://github.com/rust-osdev/bootimage) tool:
  * `cargo install bootimage`