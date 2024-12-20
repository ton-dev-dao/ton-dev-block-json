/*
 * Copyright (C) ton.dev. All Rights Reserved.
 *
 * Licensed under the SOFTWARE EVALUATION License (the "License"); you may not use
 * this file except in compliance with the License.  You may obtain a copy of the
 * License at:
 *
 * https://www.ever.dev/licenses
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific ton.dev software governing permissions and limitations
 * under the License.
 */

mod serialize;
pub use self::serialize::*;
mod block_parser;
mod deserialize;

pub use self::deserialize::*;
pub use block_parser::*;

include!("../common/src/info.rs");
