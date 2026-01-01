// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

/// Cache-padded wrapper to prevent false sharing between CPU cores.
///
/// Alignment values follow the documented L1 data cache line sizes for each
/// architecture:
/// - x86_64 and riscv64 keep 64-byte lines.
/// - Apple aarch64 parts expose 128-byte lines; other aarch64 use 64 bytes.
/// - powerpc64 reports 128-byte lines.
/// - 32-bit arm and mips flavours use 32-byte lines.
/// - s390x advertises 256-byte lines.
/// - Everything else defaults to 64 bytes.
#[cfg_attr(target_arch = "x86_64", repr(align(64)))]
#[cfg_attr(
    all(target_arch = "aarch64", target_vendor = "apple"),
    repr(align(128))
)]
#[cfg_attr(target_arch = "powerpc64", repr(align(128)))]
#[cfg_attr(target_arch = "arm", repr(align(32)))]
#[cfg_attr(
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
    ),
    repr(align(32))
)]
#[cfg_attr(target_arch = "riscv64", repr(align(64)))]
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", target_vendor = "apple"),
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "riscv64",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
pub struct CachePadded<T> {
    pub value: T,
}

impl<T> CachePadded<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
