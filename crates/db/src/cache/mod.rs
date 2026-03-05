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

use self::property_pic_stats::PropertyPicStats;
use self::stats::CacheStats;
use self::verb_pic_stats::VerbPicStats;
use std::sync::LazyLock;

pub mod ancestry_cache;
pub mod prop_cache;
pub mod property_pic_stats;
pub(crate) mod stats;
pub mod verb_cache;
pub mod verb_pic_stats;

/// Global cache statistics for property lookups.
pub static PROP_CACHE_STATS: LazyLock<CacheStats> = LazyLock::new(CacheStats::new);
/// Global cache statistics for verb lookups.
pub static VERB_CACHE_STATS: LazyLock<CacheStats> = LazyLock::new(CacheStats::new);
/// Global cache statistics for ancestry lookups.
pub static ANCESTRY_CACHE_STATS: LazyLock<CacheStats> = LazyLock::new(CacheStats::new);
/// Global PIC outcome statistics for property get/set hint paths.
pub static PROPERTY_PIC_STATS: LazyLock<PropertyPicStats> = LazyLock::new(PropertyPicStats::new);
/// Global PIC outcome statistics for verb dispatch hint paths.
pub static VERB_PIC_STATS: LazyLock<VerbPicStats> = LazyLock::new(VerbPicStats::new);
