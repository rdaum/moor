// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

object SYSOBJ [
  import_export_id -> "sysobj"
]
    name: "System Object"
    parent: ROOT
    owner: ARCH_WIZARD
    readable: true

    property arch_wizard (owner: ARCH_WIZARD, flags: "r") = ARCH_WIZARD;
    property root (owner: ARCH_WIZARD, flags: "r") = ROOT;
    property bench_controller (owner: ARCH_WIZARD, flags: "r") = BENCH_CONTROLLER;
    property bench_subscriber (owner: ARCH_WIZARD, flags: "r") = BENCH_SUBSCRIBER;
    property game_update (owner: ARCH_WIZARD, flags: "r") = GAME_UPDATE;
    property server_options (owner: ARCH_WIZARD, flags: "r") = SERVER_OPTIONS;

    verb do_login_command (this none this) owner: ARCH_WIZARD flags: "rxd"
        return ARCH_WIZARD;
    endverb

endobject
