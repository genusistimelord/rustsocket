#![crate_type = "staticlib"]

use mio::Poll;
use libc::*;
use std::{ptr, slice};
use std::boxed::Box;
use std::ffi::CStr;
use std::os::raw::c_char;

mod server;
mod client;
use client::ClientState;
use server::{Server, rust_poll_events};

#[no_mangle]
pub extern "C" fn init_socket(cpoll: *mut Poll, ipaddress: *const c_char, client_max: u64,
    recv: extern "C" fn(u64, *const c_char, u64) -> c_int,
    accept: extern "C" fn(u64, *const c_char) -> c_int,
    disconnect: extern "C" fn(u64) -> c_int) -> *mut Server {

    if cpoll.is_null() {
      return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(ipaddress) };
    let f_str = match c_str.to_str() {
        Ok(n) => n,
        Err(_) => return ptr::null_mut(),
    };

    let server = match Server::new(cpoll, f_str, client_max as usize, recv, accept, disconnect) {
        Ok(n) => n,
        Err(_) => return ptr::null_mut(),
    };

    Box::into_raw(Box::new( server ))
}

#[no_mangle]
pub extern "C" fn unload_socket(csocket: *mut Server) {
  unsafe {
    if csocket.is_null() {
      return;
    }

        Box::from_raw(csocket);
    }
}

#[no_mangle]
pub extern "C" fn init_poll() -> *mut Poll {
    let poll = match Poll::new() {
        Ok(n) => n,
        Err(_) => return ptr::null_mut(),
    };

    Box::into_raw(Box::new( poll ))
}

#[no_mangle]
pub extern "C" fn unload_poll(cpoll: *mut Poll) {
  unsafe {
      if cpoll.is_null() {
        return;
      }

      Box::from_raw(cpoll);
    }
}

#[no_mangle]
pub extern "C" fn poll_events(cpoll: *mut Poll, cserver: *mut Server) -> c_int {
  if cserver.is_null() || cpoll.is_null() {
    return -1;
  }

  match rust_poll_events(cpoll, cserver) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn socket_send(cserver: *mut Server, index: u64, data: *const c_char, size: u64) -> c_int {
    unsafe {
      if cserver.is_null() || data.is_null() {
        return -1;
      }

      let bytes = slice::from_raw_parts(data, size as usize);
      let mut v: Vec<u8> = Vec::new();

      for i in 0..size as usize {
        v.push(bytes[i] as u8);
      }

      match handle_send(&mut *cserver, index, v) {
        Ok(_) => return 0,
        Err(_) => return -1,
      }
    }
}

fn handle_send (cserver: &mut Server, index: u64, data: Vec<u8>) ->Result<(), failure::Error> {

  let client = &mut cserver.get_mut(mio::Token(index as usize)).unwrap();
  client.send(data);

  Ok(())
}

#[no_mangle]
pub extern "C" fn socket_set_interest(cpoll: *mut Poll, cserver: *mut Server, index: u64, read: bool) -> c_int {
    unsafe {

    if cserver.is_null() || cpoll.is_null() {
      return -1;
    }

    let poll = match cpoll.as_ref() {
        Some(n) => n,
        None  => return -1,
    };

    let server = &mut *cserver;

    let client = match server.get_mut(mio::Token(index as usize)) {
        Some(n) => n,
        None => return -1,
    };

    client.has_read = read;

    match client.reregister(&poll) {
        Ok(()) => 0,
        Err(_) => -1,
    }
  }
}

#[no_mangle]
pub extern "C" fn socket_close(cserver: *mut Server, index: u64) -> c_int {
  unsafe {

    if cserver.is_null() {
      return -1;
    }

    let server = &mut *cserver;

    match server.get_mut(mio::Token(index as usize)) {
      Some(a) => {
        match a.close_socket() {
          Ok(_) => {},
          Err(_) => return -1,
        };

        match a.state {
          ClientState::Closed => {
            server.remove(mio::Token(index as usize));
          }
          _ => {}
        }
      },
      None => {}
    }

    return 0;
  }
}