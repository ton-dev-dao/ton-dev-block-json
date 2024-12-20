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

use serde_json::{Map, Value};
use std::{str::FromStr, convert::TryInto};
use ton_api::{
    ton::ton_node::{rempmessagestatus, RempMessageLevel, RempMessageStatus, RempReceipt},
    IntoBoxed
};
use ton_dev_block::*;

#[allow(dead_code)]
trait ParseJson {
    fn as_uint256(&self) -> Result<UInt256>;
    fn as_base64(&self) -> Result<Vec<u8>>;
    fn as_int(&self) -> Result<i32>;
    fn as_uint(&self) -> Result<u32>;
    fn as_long(&self) -> Result<i64>;
    fn as_ulong(&self) -> Result<u64>;
}

impl ParseJson for Value {
    fn as_uint256(&self) -> Result<UInt256> {
        self.as_str().ok_or_else(|| error!("field is not str"))?.parse()
    }
    fn as_base64(&self) -> Result<Vec<u8>> {
        base64_decode(self.as_str().ok_or_else(|| error!("field is not str"))?)
    }
    fn as_int(&self) -> Result<i32> {
        match self.as_i64() {
            Some(v) => Ok(v as i32),
            None => match self.as_str() {
                Some(s) => Ok(s.parse()?),
                None => Ok(i32::default())
            }
        }
    }
    fn as_uint(&self) -> Result<u32> {
        match self.as_u64() {
            Some(v) => Ok(v as u32),
            None => match self.as_str() {
                Some(s) => Ok(s.parse()?),
                None => Ok(u32::default())
            }
        }
    }
    fn as_long(&self) -> Result<i64> {
        match self.as_i64() {
            Some(v) => Ok(v),
            None => match self.as_str() {
                Some(s) => Ok(i64::from_str(s)?),
                None => Ok(i64::default())
            }
        }
    }
    fn as_ulong(&self) -> Result<u64> {
        match self.as_u64() {
            Some(v) => Ok(v),
            None => match self.as_str() {
                Some(s) => Ok(s.parse()?),
                None => Ok(u64::default())
            }
        }
    }
}

#[derive(Debug)]
pub struct PathMap<'m, 'a> {
    map: &'m Map<String, Value>,
    path: Vec<&'a str>
}

impl<'m, 'a> PathMap<'m, 'a> {
    pub fn new(map: &'m Map<String, Value>) -> Self {
        Self {
            map,
            path: vec!["root"]
        }
    }
    pub fn cont(prev: &Self, name: &'a str, value: &'m Value) -> Result<Self> {
        let map = value
            .as_object()
            .ok_or_else(|| error!("{}/{} must be the vector of objects", prev.path.join("/"), name))?;
        let mut path = prev.path.clone();
        path.push(name);
        Ok(Self {
            map,
            path
        })
    }
    pub fn iter(&self) -> serde_json::map::Iter<'m> {
        self.map.iter()
    }
    pub fn get_item(&self, name: &'a str) -> Result<&'m Value> {
        let item = self.map.get(name).ok_or_else(|| error!("{} must have the field `{}`", self.path.join("/"), name))?;
        Ok(item)
    }
    pub fn get_obj(&self, name: &'a str) -> Result<Self> {
        let map = self.get_item(name)?
            .as_object()
            .ok_or_else(|| error!("{}/{} must be the object", self.path.join("/"), name))?;
        let mut path = self.path.clone();
        path.push(name);
        Ok(Self {
            map,
            path
        })
    }
    pub fn get_vec(&self, name: &'a str) -> Result<&'m Vec<Value>> {
        self.get_item(name)?
            .as_array()
            .ok_or_else(|| error!("{}/{} must be the vector", self.path.join("/"), name))
    }
    pub fn get_str(&self, name: &'a str) -> Result<&'m str> {
        self.get_item(name)?
            .as_str()
            .ok_or_else(|| error!("{}/{} must be the string", self.path.join("/"), name))
    }
    pub fn get_uint256(&self, name: &'a str) -> Result<UInt256> {
        self.get_str(name)?.parse()
            .map_err(|err| error!("{}/{} must be the uint256 in hex format : {}",
                self.path.join("/"), name, err))
    }
    pub fn get_base64(&self, name: &'a str) -> Result<Vec<u8>> {
        base64_decode(self.get_str(name)?)
            .map_err(|err| error!("{}/{} must be the base64 : {}",
                self.path.join("/"), name, err))
    }

    pub fn get_num(&self, name: &'a str) -> Result<i64> {
        if let Ok(value) = self.get_item(name) {
            if let Some(v) = value.as_i64() {
                return Ok(v);
            }
        }
        if let Ok(value) = self.get_item(&(name.to_string() + "_dec")) {
            if let Some(v) = value.as_str() {
                return i64::from_str(v).map_err(|err| {
                    error!(
                        "{}/{} must be the integer or a string with the integer {}: {}",
                        self.path.join("/"), name, v, err
                    )
                });
            }
        }
        if let Ok(value) = self.get_item(name) {
            if let Some(v) = value.as_str() {
                if let Some(v) = v.strip_prefix("0x") {
                    return i64::from_str_radix(v, 16).map_err(|err| {
                        error!(
                            "{}/{} must be the integer or a string with the integer {}: {}",
                            self.path.join("/"), name, v, err
                        )
                    });
                } else {
                    return i64::from_str(v).map_err(|err| {
                        error!(
                            "{}/{} must be the integer or a string with the integer {}: {}",
                            self.path.join("/"), name, v, err
                        )
                    });
                }
            }
        }
        fail!(
            "{}/{} must be the integer or a string with the integer",
            self.path.join("/"), name
        )
    }

    pub fn get_grams(&self, name: &'a str) -> Result<Grams> {
        if let Ok(value) = self.get_item(name) {
            if let Some(v) = value.as_u64() {
                return Ok(v.into());
            }
        }
        if let Ok(value) = self.get_item(&(name.to_string() + "_dec")) {
            if let Some(v) = value.as_str() {
                return Grams::from_str(v).map_err(|err| {
                    error!(
                        "{}/{} must be the integer or a string with the integer {}: {}",
                        self.path.join("/"), name, v, err
                    )
                });
            }
        }
        if let Ok(value) = self.get_item(name) {
            if let Some(v) = value.as_str() {
                return Grams::from_str(v).map_err(|err| {
                    error!(
                        "{}/{} must be the integer or a string with the integer {}: {}",
                        self.path.join("/"), name, v, err
                    )
                });
            }
        }
        fail!(
            "{}/{} must be the integer or a string with the integer",
            self.path.join("/"), name
        )
    }

    pub fn get_u32(&self, name: &'a str, value: &mut u32) {
        if let Ok(new_value) = self.get_num(name) {
            *value = new_value as u32;
        }
    }
    pub fn get_u16(&self, name: &'a str, value: &mut u16) {
        if let Ok(new_value) = self.get_num(name) {
            *value = new_value as u16;
        }
    }
    pub fn get_u8(&self, name: &'a str, value: &mut u8) {
        if let Ok(new_value) = self.get_num(name) {
            *value = new_value as u8;
        }
    }
    pub fn get_num16(&self, name: &'a str) -> Result<u16> {
        Ok(self.get_num(name)? as u16)
    }
    pub fn get_bool(&self, name: &'a str) -> Result<bool> {
        self.get_item(name)?
            .as_bool()
            .ok_or_else(|| error!("{}/{} must be boolean", self.path.join("/"), name))
    }
}

struct StateParser {
    state: ShardStateUnsplit,
    extra: McStateExtra,
    mandatory_params: u64
}

impl StateParser {

    fn new() -> Self {
        Self {
            state: ShardStateUnsplit::with_ident(ShardIdent::masterchain()),
            extra: McStateExtra::default(),
            mandatory_params: 0,
        }
    }

    fn for_zero_state() -> Self {
        // let mandatory_params = [0, 1, 2, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18,
        //     20, 21, 22, 23, 24, 25, 28, 29, 31, 34];
        // let mandatory_params = mandatory_params.iter().fold(0, |s, p| a |= 1 << p);
        // println!("0x{:X}", mandatory_params);
        Self {
            state: ShardStateUnsplit::with_ident(ShardIdent::masterchain()),
            extra: McStateExtra::default(),
            mandatory_params: 0x0000_0004_B3F7_CF87,
        }
    }

    fn is_need(&self, num: i32) -> bool {
        ((self.mandatory_params >> num) & 1) != 0
    }

    fn parse_parameter(
        &mut self,
        config: &PathMap,
        num: i32,
        f: impl FnOnce(&PathMap) -> Result<ConfigParamEnum>
    ) -> Result<()> {
        let p = format!("p{}", num);
        match config.get_obj(&p) {
            Ok(p) => {
                self.extra.config.set_config(f(&p)?)
                    .map_err(|err| error!("Can't set config for {} : {}", p.path.join("/"), err))
            }
            Err(err) if self.is_need(num) => {
                fail!("parameter p{} not found: {}", num, err)
            }
            _ => Ok(())
        }
    }

    fn parse_array(
        &mut self,
        config: &PathMap,
        num: i32,
        f: impl FnOnce(&Vec<Value>) -> Result<ConfigParamEnum>
    ) -> Result<()> {
        let p = format!("p{}", num);
        match config.get_vec(&p) {
            Ok(v) => {
                self.extra.config.set_config(f(v)?)
                    .map_err(|err| error!("Can't set config for {} : {}", config.path.join("/"), err))
            }
            Err(err) if self.is_need(num) => {
                fail!("parameter p{} not found: {}", num, err)
            }
            _ => Ok(())
        }
    }

    fn parse_uint256(
        &mut self,
        config: &PathMap,
        num: i32,
        f: impl FnOnce(UInt256) -> Result<ConfigParamEnum>
    ) -> Result<()> {
        let p = format!("p{}", num);
        match config.get_uint256(&p) {
            Ok(p) => {
                self.extra.config.set_config(f(p)?)
                    .map_err(|err| error!("Can't set config for {} : {}", config.path.join("/"), err))
            }
            Err(err) if self.is_need(num) => {
                fail!("parameter p{} not found: {}", num, err)
            }
            _ => Ok(())
        }
    }

    fn parse_param_set_params(&mut self, config: &PathMap, num: i32) -> Result<Option<MandatoryParams>> {
        let p = format!("p{}", num);
        match config.get_vec(&p) {
            Ok(vec) => {
                let mut params = MandatoryParams::default();
                vec.iter().try_for_each(|n| params.add_key(&n.as_uint()?))?;
                Ok(Some(params))
            }
            Err(err) => {
                if self.is_need(num) {
                    Err(err)
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn parse_param_limits(param: &PathMap) -> Result<ParamLimits> {
        ParamLimits::with_limits(
            param.get_num("underload")? as u32,
            param.get_num("soft_limit")? as u32,
            param.get_num("hard_limit")? as u32,
        )
    }

    fn parse_block_limits_struct(param: &PathMap) -> Result<BlockLimits> {
        Ok(BlockLimits::with_limits(
            Self::parse_param_limits(&param.get_obj("bytes")?)?,
            Self::parse_param_limits(&param.get_obj("gas")?)?,
            Self::parse_param_limits(&param.get_obj("lt_delta")?)?,
        ))
    }

    fn parse_block_limits(&mut self, config: &PathMap) -> Result<()> {
        self.parse_parameter(config, 22, |p| Ok(ConfigParamEnum::ConfigParam22(Self::parse_block_limits_struct(p)?)))?;
        self.parse_parameter(config, 23, |p| Ok(ConfigParamEnum::ConfigParam23(Self::parse_block_limits_struct(p)?)))
    }

    fn parse_msg_forward_prices_struct(param: &PathMap) -> Result<MsgForwardPrices> {
        Ok(MsgForwardPrices {
            lump_price:       param.get_num("lump_price")? as u64,
            bit_price:        param.get_num("bit_price")? as u64,
            cell_price:       param.get_num("cell_price")? as u64,
            ihr_price_factor: param.get_num("ihr_price_factor")? as u32,
            first_frac:       param.get_num("first_frac")? as u16,
            next_frac:        param.get_num("next_frac")? as u16,
        })
    }

    fn parse_msg_forward_prices(&mut self, config: &PathMap) -> Result<()> {
        self.parse_parameter(config, 24, |p| Ok(ConfigParamEnum::ConfigParam24(Self::parse_msg_forward_prices_struct(p)?)))?;
        self.parse_parameter(config, 25, |p| Ok(ConfigParamEnum::ConfigParam25(Self::parse_msg_forward_prices_struct(p)?)))
    }

    fn parse_gas_limits_struct(param: &PathMap) -> Result<GasLimitsPrices> {
        Ok(GasLimitsPrices {
            gas_price:         param.get_num("gas_price")? as u64,
            gas_limit:         param.get_num("gas_limit")? as u64,
            special_gas_limit: param.get_num("special_gas_limit")? as u64,
            gas_credit:        param.get_num("gas_credit")? as u64,
            block_gas_limit:   param.get_num("block_gas_limit")? as u64,
            freeze_due_limit:  param.get_num("freeze_due_limit")? as u64,
            delete_due_limit:  param.get_num("delete_due_limit")? as u64,
            flat_gas_limit:    param.get_num("flat_gas_limit")? as u64,
            flat_gas_price:    param.get_num("flat_gas_price")? as u64,
            max_gas_threshold: 0,
        })
    }

    fn parse_gas_limits(&mut self, config: &PathMap) -> Result<()> {
        self.parse_parameter(config, 20, |p| Ok(ConfigParamEnum::ConfigParam20(Self::parse_gas_limits_struct(p)?)))?;
        self.parse_parameter(config, 21, |p| Ok(ConfigParamEnum::ConfigParam21(Self::parse_gas_limits_struct(p)?)))
    }

    fn parse_storage_prices(&mut self, config: &PathMap) -> Result<()> {
        self.parse_array(config, 18, |p18| {
            let mut map = ConfigParam18Map::default();
            let mut index = 0u32;
            p18.iter().try_for_each::<_, Result<_>>(|value| {
                let p = PathMap::cont(config, "p18", value)?;
                let p = StoragePrices {
                    utime_since:      p.get_num("utime_since")? as u32,
                    bit_price_ps:     p.get_num("bit_price_ps")? as u64,
                    cell_price_ps:    p.get_num("cell_price_ps")? as u64,
                    mc_bit_price_ps:  p.get_num("mc_bit_price_ps")? as u64,
                    mc_cell_price_ps: p.get_num("mc_cell_price_ps")? as u64,
                };
                map.set(&index, &p)?;
                index += 1;
                Ok(())
            })?;
            Ok(ConfigParamEnum::ConfigParam18(ConfigParam18 { map }))
        })
    }

    fn parse_mesh_config(&mut self, config: &PathMap) -> Result<()> {
        self.parse_array(config, 58, |p58| {
            let mut map = MeshConfig::default();
            p58.iter().try_for_each::<_, Result<_>>(|value| {
                let p = PathMap::cont(config, "p58", value)?;
                let nw_id = p.get_num("network_id")? as i32;
                let mut hardforks = vec!();
                if let Ok(vector) = p.get_vec("hardforks") {
                    for hf in vector {
                        let p = PathMap::cont(&p, "hardforks", hf)?;
                        hardforks.push(parse_separated_block_id_ext(&p)?);
                    }
                }
                let nw_cfg = ConnectedNwConfig {
                    zerostate: parse_separated_block_id_ext(&p.get_obj("zerostate")?)?,
                    is_active: p.get_bool("is_active")?,
                    currency_id: p.get_num("currency_id")? as u32,
                    init_block: parse_separated_block_id_ext(&p.get_obj("init_block")?)?,
                    emergency_guard_addr: p.get_uint256("emergency_guard_addr")?,
                    pull_addr: p.get_uint256("pull_addr")?,
                    minter_addr: p.get_uint256("minter_addr")?,
                    hardforks
                };
                map.set(&nw_id, &nw_cfg)?;
                Ok(())
            })?;
            Ok(ConfigParamEnum::ConfigParam58(map))
        })
    }

    fn parse_param_set(&mut self, config: &PathMap) -> Result<()> {
        if let Some(mandatory_params) = self.parse_param_set_params(config, 9)? {
            self.extra.config.set_config(ConfigParamEnum::ConfigParam9(ConfigParam9 {mandatory_params} ))?;
        }
        if let Some(critical_params) = self.parse_param_set_params(config, 10)? {
            self.extra.config.set_config(ConfigParamEnum::ConfigParam10(ConfigParam10 {critical_params} ))?;
        }
        Ok(())
    }

    fn parse_critical_params(params: &PathMap) -> Result<ConfigProposalSetup> {
        Ok(ConfigProposalSetup {
            min_tot_rounds: params.get_num("min_tot_rounds")? as u8,
            max_tot_rounds: params.get_num("max_tot_rounds")? as u8,
            min_wins      : params.get_num("min_wins"      )? as u8,
            max_losses    : params.get_num("max_losses"    )? as u8,
            min_store_sec : params.get_num("min_store_sec" )? as u32,
            max_store_sec : params.get_num("max_store_sec" )? as u32,
            bit_price     : params.get_num("bit_price"     )? as u32,
            cell_price    : params.get_num("cell_price"    )? as u32,
        })
    }

    fn parse_p11(&mut self, config: &PathMap) -> Result<()> {
        self.parse_parameter(config, 11, |p11| {
            let normal_params = Self::parse_critical_params(&p11.get_obj("normal_params")?)?;
            let critical_params = Self::parse_critical_params(&p11.get_obj("critical_params")?)?;
            let p11 = ConfigParam11::new(&normal_params, &critical_params)?;
            Ok(ConfigParamEnum::ConfigParam11(p11))
        })
    }

    fn parse_p12(&mut self, config: &PathMap) -> Result<()> {
        self.parse_array(config, 12, |p12| {
            let mut workchains = Workchains::default();
            p12.iter().try_for_each(|wc_info| {
                let wc_info = PathMap::cont(config, "p12", wc_info)?;
                let mut descr = WorkchainDescr::default();
                let workchain_id = wc_info.get_num("workchain_id")? as u32;
                descr.enabled_since = wc_info.get_num("enabled_since")? as u32;
                descr.set_min_split(wc_info.get_num("min_split")? as u8)?;
                descr.set_max_split(wc_info.get_num("max_split")? as u8)?;
                descr.flags = wc_info.get_num("flags")? as u16;
                descr.active = wc_info.get_bool("active")?;
                descr.accept_msgs = wc_info.get_bool("accept_msgs")?;
                descr.zerostate_root_hash = wc_info.get_uint256("zerostate_root_hash")?;
                descr.zerostate_file_hash = wc_info.get_uint256("zerostate_file_hash")?;
                descr.version = wc_info.get_num("version")? as u32;
                // TODO: check here
                descr.format = match wc_info.get_bool("basic")? {
                    true => {
                        let vm_version = wc_info.get_num("vm_version")? as i32;
                        let vm_mode    = wc_info.get_num("vm_mode"   )? as u64;
                        WorkchainFormat::Basic(WorkchainFormat1::with_params(vm_version, vm_mode))
                    }
                    false => {
                        let min_addr_len      = wc_info.get_num("min_addr_len")? as u16;
                        let max_addr_len      = wc_info.get_num("max_addr_len")? as u16;
                        let addr_len_step     = wc_info.get_num("addr_len_step")? as u16;
                        let workchain_type_id = wc_info.get_num("workchain_type_id")? as u32;
                        WorkchainFormat::Extended(
                            WorkchainFormat0::with_params(
                                min_addr_len, max_addr_len, addr_len_step, workchain_type_id
                            )?
                        )
                    }
                };
                workchains.set(&workchain_id, &descr)
            })?;
            Ok(ConfigParamEnum::ConfigParam12(ConfigParam12 {workchains}))
        })
    }

    fn parse_catchain_config(p28: &PathMap) -> Result<ConfigParamEnum> {
        Ok(ConfigParamEnum::ConfigParam28(CatchainConfig {
            shuffle_mc_validators:     p28.get_bool("shuffle_mc_validators")?,
            isolate_mc_validators:     p28.get_bool("isolate_mc_validators").unwrap_or_default(),
            mc_catchain_lifetime:      p28.get_num("mc_catchain_lifetime")? as u32,
            shard_catchain_lifetime:   p28.get_num("shard_catchain_lifetime")? as u32,
            shard_validators_lifetime: p28.get_num("shard_validators_lifetime")? as u32,
            shard_validators_num:      p28.get_num("shard_validators_num")? as u32,
        }))
    }

    fn parse_consensus_config(p29: &PathMap) -> Result<ConfigParamEnum> {
        Ok(ConfigParamEnum::ConfigParam29(ConfigParam29 {consensus_config: ConsensusConfig {
            new_catchain_ids:        p29.get_bool("new_catchain_ids")?,
            round_candidates:        p29.get_num("round_candidates")? as u32,
            next_candidate_delay_ms: p29.get_num("next_candidate_delay_ms")? as u32,
            consensus_timeout_ms:    p29.get_num("consensus_timeout_ms")? as u32,
            fast_attempts:           p29.get_num("fast_attempts")? as u32,
            attempt_duration:        p29.get_num("attempt_duration")? as u32,
            catchain_max_deps:       p29.get_num("catchain_max_deps")? as u32,
            max_block_bytes:         p29.get_num("max_block_bytes")? as u32,
            max_collated_bytes:      p29.get_num("max_collated_bytes")? as u32,
        }}))
    }

    fn parse_delector_params(p30: &PathMap) -> Result<ConfigParamEnum> {
        Ok(ConfigParamEnum::ConfigParam30(DelectorParams {
            delections_step         : p30.get_num("delections_step")? as u32,
            validator_init_code_hash: p30.get_uint256("validator_init_code_hash")?,
            staker_init_code_hash   : p30.get_uint256("staker_init_code_hash")?,
        }))
    }

    fn parse_smft_params(p62: &PathMap) -> Result<ConfigParamEnum> {
        Ok(ConfigParamEnum::ConfigParam62(SmftParams {
            min_forwarding_neighbours_count: p62.get_num("min_forwarding_neighbours_count")? as u32,
            max_forwarding_neighbours_count: p62.get_num("max_forwarding_neighbours_count")? as u32,
            min_far_neighbours_count: p62.get_num("min_far_neighbours_count")? as u32,
            max_far_neighbours_count: p62.get_num("max_far_neighbours_count")? as u32,
            min_block_sync_period_ms: p62.get_num("min_block_sync_period_ms")? as u32,
            max_block_sync_period_ms: p62.get_num("max_block_sync_period_ms")? as u32,
            min_far_neighbours_sync_period_ms: p62.get_num("min_far_neighbours_sync_period_ms")? as u32,
            max_far_neighbours_sync_period_ms: p62.get_num("max_far_neighbours_sync_period_ms")? as u32,
            far_neighbours_resync_period_ms: p62.get_num("far_neighbours_resync_period_ms")? as u32,
            block_sync_lifetime_period_ms: p62.get_num("block_sync_lifetime_period_ms")? as u32,
            block_lifetime_period_ms: p62.get_num("block_lifetime_period_ms")? as u32,
            verification_obligation_cutoff_numerator: p62.get_num("verification_obligation_cutoff_numerator")? as u32,
            verification_obligation_cutoff_denominator: p62.get_num("verification_obligation_cutoff_denominator")? as u32,
            delivery_cutoff_numerator: p62.get_num("delivery_cutoff_numerator")? as u32,
            delivery_cutoff_denominator: p62.get_num("delivery_cutoff_denominator")? as u32,
            manual_candidate_loading_delay_ms: p62.get_num("manual_candidate_loading_delay_ms")? as u32,
            mc_allowed_force_delivery_delay_ms: p62.get_num("mc_allowed_force_delivery_delay_ms")? as u32,
            mc_force_delivery_duplication_factor_numerator: p62.get_num("mc_force_delivery_duplication_factor_numerator")? as u32,
            mc_force_delivery_duplication_factor_denominator: p62.get_num("mc_force_delivery_duplication_factor_denominator")? as u32,
            mc_max_delivery_waiting_timeout_ms: p62.get_num("mc_max_delivery_waiting_timeout_ms")? as u32,
            use_debug_bls_keys: p62.get_bool("use_debug_bls_keys")?,
        }))
    }

    fn parse_validator_set(config: &PathMap) -> Result<ValidatorSet> {
        let utime_since = config.get_num("utime_since")? as u32;
        let utime_until = config.get_num("utime_until")? as u32;
        //let total = config.get_num("total")? as u16;
        let main = config.get_num("main")? as u16;
        //let total_weight = config.get_num("total_weight")? as u64;

        let mut list = Vec::default();
        config.get_vec("list").and_then(|p| {
            p.iter().try_for_each::<_, Result<_>>(|p| {
                let p = PathMap::cont(config, "p", p)?;
                let public_key = hex::decode(p.get_str("public_key")?)?;
                let weight = p.get_num("weight")? as u64;
                let adnl_addr = if let Ok(adnl_addr) = p.get_uint256("adnl_addr") {
                    Some(adnl_addr)
                } else {
                    None
                };
                let bls_public_key = if let Ok(bls_public_key) = p.get_str("bls_public_key") {
                    let bls_public_key = hex::decode(bls_public_key)?;
                    Some(bls_public_key.as_slice().try_into()?)
                } else { 
                    None 
                };

                let descr = ValidatorDescr::with_params(
                    SigPubKey::from_bytes(&public_key)?,
                    weight,
                    adnl_addr,
                    bls_public_key
                );
                list.push(descr);

                Ok(())
            })?;
            Ok(())
        })?;

        let validator_set = ValidatorSet::new(utime_since, utime_until, main, list)?;
        Ok(validator_set)
    }

    pub fn parse_config(&mut self, config: &PathMap) -> Result<()> {
        self.parse_uint256(config, 0, |config_addr | Ok(ConfigParamEnum::ConfigParam0(ConfigParam0 {config_addr} )))?;
        self.parse_uint256(config, 1, |elector_addr| Ok(ConfigParamEnum::ConfigParam1(ConfigParam1 {elector_addr} )))?;
        self.parse_uint256(config, 2, |minter_addr | Ok(ConfigParamEnum::ConfigParam2(ConfigParam2 {minter_addr} )))?;
        self.parse_uint256(config, 3, |fee_collector_addr | Ok(ConfigParamEnum::ConfigParam3(ConfigParam3 {fee_collector_addr} )))?;
        self.parse_uint256(config, 4, |dns_root_addr | Ok(ConfigParamEnum::ConfigParam4(ConfigParam4 {dns_root_addr} )))?;
        self.parse_uint256(config, 5, |owner_addr | Ok(ConfigParamEnum::ConfigParam5(ConfigParam5 {owner_addr} )))?;

        self.parse_parameter(config, 6, |value| {
            Ok(ConfigParamEnum::ConfigParam6(ConfigParam6 {
                mint_new_price: value.get_grams("mint_new_price")?,
                mint_add_price: value.get_grams("mint_add_price")?,
            }))
        })?;

        self.parse_array(config, 7, |p7| {
            let mut to_mint = ExtraCurrencyCollection::default();
            p7.iter().try_for_each(|currency| {
                let currency = PathMap::cont(config, "p7", currency)?;
                let value = if let Ok(value) = currency.get_str("value_dec") {
                    value.parse()?
                } else {
                    currency.get_str("value")?.parse()?
                };
                to_mint.set(
                    &(currency.get_num("currency")? as u32),
                    &value
                )
            })?;
            Ok(ConfigParamEnum::ConfigParam7(ConfigParam7 {to_mint} ))
        })?;

        self.parse_parameter(config, 8, |p8| {
            Ok(ConfigParamEnum::ConfigParam8(ConfigParam8 { global_version: GlobalVersion {
                version: p8.get_num("version")? as u32,
                capabilities: p8.get_num("capabilities")? as u64,
            }}))
        })?;

        self.parse_param_set(config)?; // p9 p10
        self.parse_p11(config)?;
        self.parse_p12(config)?;

        self.parse_parameter(config, 13, |p13| {
            let cell = read_single_root_boc(p13.get_base64("boc")?)?;
            Ok(ConfigParamEnum::ConfigParam13(ConfigParam13 { cell }))
        })?;
        self.parse_parameter(config, 14, |p14| {
            Ok(ConfigParamEnum::ConfigParam14(ConfigParam14 {
                block_create_fees: BlockCreateFees {
                    masterchain_block_fee: p14.get_grams("masterchain_block_fee")?,
                    basechain_block_fee: p14.get_grams("basechain_block_fee")?,
            }}))
        })?;

        self.parse_parameter(config, 15, |p15| {
            Ok(ConfigParamEnum::ConfigParam15(ConfigParam15 {
                validators_elected_for: p15.get_num("validators_elected_for")? as u32,
                elections_start_before: p15.get_num("elections_start_before")? as u32,
                elections_end_before:   p15.get_num("elections_end_before")? as u32,
                stake_held_for:         p15.get_num("stake_held_for")? as u32,
            }))
        })?;

        self.parse_parameter(config, 16, |p16| {
            Ok(ConfigParamEnum::ConfigParam16(ConfigParam16 {
                min_validators:      p16.get_num16("min_validators")?.into(),
                max_validators:      p16.get_num16("max_validators")?.into(),
                max_main_validators: p16.get_num16("max_main_validators")?.into(),
            }))
        })?;

        self.parse_parameter(config, 17, |p17| {
            Ok(ConfigParamEnum::ConfigParam17(ConfigParam17 {
                min_stake:        p17.get_grams("min_stake")?,
                max_stake:        p17.get_grams("max_stake")?,
                min_total_stake:  p17.get_grams("min_total_stake")?,
                max_stake_factor: p17.get_num("max_stake_factor")? as u32,
            }))
        })?;

        self.parse_storage_prices(config)?;     // p18
        self.parse_gas_limits(config)?;        // p20 p21
        self.parse_block_limits(config)?;      // p22 p23
        self.parse_msg_forward_prices(config)?;// p24 p25
        self.parse_parameter(config, 28, Self::parse_catchain_config)?;
        self.parse_parameter(config, 29, Self::parse_consensus_config)?;
        self.parse_parameter(config, 30, Self::parse_delector_params)?;

        self.parse_array(config, 31, |p31| {
            let mut fundamental_smc_addr = FundamentalSmcAddresses::default();
            p31.iter().try_for_each(|n| fundamental_smc_addr.add_key(&n.as_uint256()?))?;
            Ok(ConfigParamEnum::ConfigParam31(ConfigParam31 {fundamental_smc_addr} ))
        })?;

        self.parse_parameter(config, 32, |p| Ok(ConfigParamEnum::ConfigParam32(ConfigParam32{prev_validators: Self::parse_validator_set(p)?})))?;
        self.parse_parameter(config, 33, |p| Ok(ConfigParamEnum::ConfigParam33(ConfigParam33{prev_temp_validators: Self::parse_validator_set(p)?})))?;

        self.parse_parameter(config, 34, |p34| {
            let mut list = vec![];
            p34.get_vec("list").and_then(|p| p.iter().try_for_each::<_, Result<()>>(|p| {
                let p = PathMap::cont(config, "p34", p)?;
                let bls_public_key = if let Ok(bls_public_key) = p.get_str("bls_public_key") {
                    if bls_public_key.len() != 96 {
                        fail!("Invalid BLS public key length {}", bls_public_key.len());
                    }
                    let bls_public_key = hex::decode(bls_public_key)?;
                    Some(bls_public_key.as_slice().try_into()?)
                } else {
                    None
                };

                list.push(ValidatorDescr::with_params(
                    p.get_str("public_key")?.parse()?,
                    p.get_num("weight")? as u64,
                    None,
                    bls_public_key,
                ));
                Ok(())
            }))?;
            let cur_validators = ValidatorSet::new(
                p34.get_num("utime_since")? as u32,
                p34.get_num("utime_until")? as u32,
                p34.get_num("main")? as u16,
                list
            )?;
            Ok(ConfigParamEnum::ConfigParam34(ConfigParam34 {cur_validators}))
        })?;

        self.parse_parameter(config, 35, |p| Ok(ConfigParamEnum::ConfigParam35(ConfigParam35{cur_temp_validators: Self::parse_validator_set(p)?})))?;
        self.parse_parameter(config, 36, |p| Ok(ConfigParamEnum::ConfigParam36(ConfigParam36{next_validators: Self::parse_validator_set(p)?})))?;
        self.parse_parameter(config, 37, |p| Ok(ConfigParamEnum::ConfigParam37(ConfigParam37{next_temp_validators: Self::parse_validator_set(p)?})))?;

        self.parse_array(config, 39, |p39| {
            let mut validator_keys = ValidatorKeys::default();

            p39.iter().try_for_each::<_, Result<()>>(|p| {
                let p = PathMap::cont(config, "p39", p)?;

                let key = p.get_uint256("map_key")?;
                let adnl_addr = p.get_uint256("adnl_addr")?;
                let temp_public_key = hex::decode(p.get_str("temp_public_key")?)?;
                let seqno = p.get_num("seqno")? as u32;
                let valid_until = p.get_num("valid_until")? as u32;
                let signature_r = p.get_str("signature_r")?;
                let signature_s = p.get_str("signature_s")?;

                let pk = ValidatorTempKey::with_params(
                    adnl_addr,
                    SigPubKey::from_bytes(&temp_public_key)?,
                    seqno,
                    valid_until,
                );
                let sk = CryptoSignature::from_r_s_str(signature_r, signature_s)?;
                validator_keys.set(&key, &ValidatorSignedTempKey::with_key_and_signature(pk, sk))?;
                Ok(())
            })?;

            Ok(ConfigParamEnum::ConfigParam39(ConfigParam39 { validator_keys }))
        })?;

        let mut slashing_config = SlashingConfig::default();
        if let Ok(p40) = config.get_obj("p40") {
            p40.get_u32("slashing_period_mc_blocks_count", &mut slashing_config.slashing_period_mc_blocks_count);
            p40.get_u32("resend_mc_blocks_count", &mut slashing_config.resend_mc_blocks_count);
            p40.get_u32("min_samples_count", &mut slashing_config.min_samples_count);
            p40.get_u32("collations_score_weight", &mut slashing_config.collations_score_weight);
            p40.get_u32("signing_score_weight", &mut slashing_config.signing_score_weight);
            p40.get_u32("min_slashing_protection_score", &mut slashing_config.min_slashing_protection_score);
            p40.get_u32("z_param_numerator", &mut slashing_config.z_param_numerator);
            p40.get_u32("z_param_denominator", &mut slashing_config.z_param_denominator);
        }
        self.extra.config.set_config(ConfigParamEnum::ConfigParam40(ConfigParam40 {slashing_config}))?;

        self.parse_parameter(config, 42, |p42| {
            let mut copyleft_config = ton_dev_block::ConfigCopyleft {
                copyleft_reward_threshold: p42.get_grams("threshold")?,
                ..Default::default() 
            };
            p42.get_vec("payouts").and_then(|p| {
                p.iter().try_for_each::<_, Result<()>>(|p| {
                    let p = PathMap::cont(config, "p42", p)?;
                    let mut license_type = 0;
                    p.get_u32("license_type", &mut license_type);
                    let mut percent = 0;
                    p.get_u32("payout_percent", &mut percent);
                    copyleft_config.license_rates.set(&(license_type as u8), &(percent as u8))?;
                    Ok(())
                })
            })?;
            Ok(ConfigParamEnum::ConfigParam42(copyleft_config))
        })?;

        self.parse_array(config, 44, |p44| {
            let mut suspended = SuspendedAddresses::new();

            for address in p44 {
                let address: MsgAddressInt = address
                    .as_str()
                    .ok_or_else(|| error!("address must be string"))?.parse()?;
                suspended.add_suspended_address(
                    address.get_workchain_id(),
                    UInt256::construct_from(&mut address.address())?
                )?;
            }

            Ok(ConfigParamEnum::ConfigParam44(suspended))
        })?;

        self.parse_mesh_config(config)?;     // p58

        if let Ok(p61) = config.get_obj("p61") {
            let mut ff_config = FastFinalityConfig::default();
            p61.get_u32("split_merge_interval", &mut ff_config.split_merge_interval);
            p61.get_u32("collator_range_len", &mut ff_config.collator_range_len);
            p61.get_u32("lost_collator_timeout", &mut ff_config.lost_collator_timeout);
            p61.get_u32("mempool_validators_count", &mut ff_config.mempool_validators_count);
            p61.get_u32("mempool_rotated_count", &mut ff_config.mempool_rotated_count);
            p61.get_u16("unreliability_fine", &mut ff_config.unreliability_fine);
            p61.get_u16("unreliability_weak_fading", &mut ff_config.unreliability_weak_fading);
            p61.get_u16("unreliability_strong_fading", &mut ff_config.unreliability_strong_fading);
            p61.get_u16("unreliability_max", &mut ff_config.unreliability_max);
            p61.get_u16("unreliability_weight", &mut ff_config.unreliability_weight);
            p61.get_u16("familiarity_collator_fine", &mut ff_config.familiarity_collator_fine);
            p61.get_u16("familiarity_msgpool_fine", &mut ff_config.familiarity_msgpool_fine);
            p61.get_u16("familiarity_fading", &mut ff_config.familiarity_fading);
            p61.get_u16("familiarity_max", &mut ff_config.familiarity_max);
            p61.get_u16("familiarity_weight", &mut ff_config.familiarity_weight);
            p61.get_u16("busyness_collator_fine", &mut ff_config.busyness_collator_fine);
            p61.get_u16("busyness_msgpool_fine", &mut ff_config.busyness_msgpool_fine);
            p61.get_u16("busyness_weight", &mut ff_config.busyness_weight);
            p61.get_u8("candidates_percentile", &mut ff_config.candidates_percentile);
            self.extra.config.set_config(ConfigParamEnum::ConfigParam61(ff_config))?;
        }

        self.parse_parameter(config, 62, Self::parse_smft_params)?;

        Ok(())
    }

    fn parse_state_unchecked(mut self, map: &Map<String, Value>) -> Result<ShardStateUnsplit> {
        let map_path = PathMap::new(map);

        self.state.set_min_ref_mc_seqno(u32::MAX);

        match map_path.get_num("global_id") {
            Ok(global_id) => self.state.set_global_id(global_id as i32),
            Err(err) => {
                if self.mandatory_params != 0 {
                    return Err(err)
                }
            }
        }
        match map_path.get_num("gen_utime") {
            Ok(gen_utime) => self.state.set_gen_time(gen_utime as u32),
            Err(err) => {
                if self.mandatory_params != 0 {
                    return Err(err)
                }
            }
        }

        match map_path.get_grams("total_balance") {
            Ok(balance) => self.state.set_total_balance(CurrencyCollection::from_grams(balance)),
            Err(err) => {
                if self.mandatory_params != 0 {
                    return Err(err)
                }
            }
        }

        match map_path.get_obj("master") {
            Ok(master) => {
                let config = master.get_obj("config")?;
                self.parse_config(&config)?;
                match master.get_uint256("config_addr") {
                    Ok(addr) => self.extra.config.config_addr = addr,
                    Err(err) => {
                        if self.mandatory_params != 0 {
                            return Err(err)
                        }
                    }
                }
                match master.get_num("validator_list_hash_short") {
                    Ok(v) => self.extra.validator_info.validator_list_hash_short = v as u32,
                    Err(err) => {
                        if self.mandatory_params != 0 {
                            return Err(err)
                        }
                    }
                }
                match master.get_num("catchain_seqno") {
                    Ok(v) => self.extra.validator_info.catchain_seqno = v as u32,
                    Err(err) => {
                        if self.mandatory_params != 0 {
                            return Err(err)
                        }
                    }
                }
                match master.get_bool("nx_cc_updated") {
                    Ok(v) => self.extra.validator_info.nx_cc_updated = v,
                    Err(err) => {
                        if self.mandatory_params != 0 {
                            return Err(err)
                        }
                    }
                }
                match master.get_grams("global_balance") {
                    Ok(balance) => self.extra.global_balance.grams = balance,
                    Err(err) => {
                        if self.mandatory_params != 0 {
                            return Err(err)
                        }
                    }
                }
                self.extra.after_key_block = true;
                self.state.write_custom(Some(&self.extra))?;
            }
            Err(err) => {
                if self.mandatory_params != 0 {
                    return Err(err)
                }
            }
        }

        if let Ok(accounts) = map_path.get_vec("accounts") {
            let mut shard_accounts = self.state.read_accounts()?;
            accounts.iter().try_for_each::<_, Result<()>>(|account| {
                let account = PathMap::cont(&map_path, "accounts", account)?;
                let account = Account::construct_from_bytes(&account.get_base64("boc")?)?;
                if let Some(account_id) = account.get_id() {
                    let aug = account.aug()?;
                    let account = ShardAccount::with_params(&account, UInt256::ZERO, 0)?;
                    shard_accounts.set_builder_serialized(
                        account_id,
                        &account.write_to_new_cell()?,
                        &aug
                    )?;
                }
                Ok(())
            })?;
            self.state.write_accounts(&shard_accounts)?;
        }

        if let Ok(libraries) = map_path.get_vec("libraries") {
            libraries.iter().try_for_each::<_, Result<()>>(|library| {
                let library = PathMap::cont(&map_path, "libraries", library)?;
                let id = library.get_uint256("hash")?;
                let lib = library.get_base64("lib")?;
                let lib = read_single_root_boc(lib)?;
                let mut lib = LibDescr::new(lib);
                let publishers = library.get_vec("publishers")?;
                publishers.iter().try_for_each::<_, Result<()>>(|publisher| {
                    lib.publishers_mut().add_key(&publisher.as_uint256()?)
                })?;
                self.state.libraries_mut().set(&id, &lib)?;
                Ok(())
            })?;
        }

        Ok(self.state)
    }
}

pub fn parse_config_with_mandatory_params(config: &Map<String, Value>, mandatories: &[u32]) -> Result<ConfigParams> {
    let config = PathMap::new(config);
    let mut parser = StateParser::new();
    if !mandatories.is_empty() {
        parser.mandatory_params = 0;
        for mandatory in mandatories {
            parser.mandatory_params |= 1u64 << mandatory;
        }
    }
    parser.parse_config(&config)?;
    Ok(parser.extra.config)
}

pub fn parse_config(config: &Map<String, Value>) -> Result<ConfigParams> {
    parse_config_with_mandatory_params(config, &[])
}

pub fn parse_state(map: &Map<String, Value>) -> Result<ShardStateUnsplit> {
    StateParser::for_zero_state().parse_state_unchecked(map)
}

pub fn parse_state_unchecked(map: &Map<String, Value>) -> Result<ShardStateUnsplit> {
    StateParser::new().parse_state_unchecked(map)
}

fn parse_block_id_ext(map_path: &PathMap, mc: bool) -> Result<BlockIdExt> {
    if mc {
        Ok(BlockIdExt::with_params(
            ton_dev_block::ShardIdent::with_tagged_prefix(MASTERCHAIN_ID, SHARD_FULL)?,
            map_path.get_num("mc_block_seqno")? as u32,
            map_path.get_uint256("mc_block_id")?,
            map_path.get_uint256("mc_block_file_hash")?,
        ))
    } else {
        Ok(BlockIdExt::with_params(
            ton_dev_block::ShardIdent::with_tagged_prefix(
                map_path.get_num("wc")? as i32,
                u64::from_str_radix(map_path.get_str("shard")?, 16)?
            )?,
            map_path.get_num("block_seqno")? as u32,
            map_path.get_uint256("block_id")?,
            map_path.get_uint256("block_file_hash")?,
        ))
    }
}

fn parse_separated_block_id_ext(map_path: &PathMap) -> Result<BlockIdExt> {
    Ok(BlockIdExt::with_params(
        ShardIdent::with_tagged_prefix(
            map_path.get_num("wc")? as i32,
            u64::from_str_radix(map_path.get_str("shard")?, 16)?
        )?,
        map_path.get_num("seqno")? as u32,
        map_path.get_uint256("root_hash")?,
        map_path.get_uint256("file_hash")?,
    ))
}

pub fn parse_remp_status(map: &Map<String, Value>)
    -> Result<(RempReceipt, Vec<u8>)> {

    let map_path = PathMap::new(map);

    let source_id = map_path.get_uint256("source_id")?;
    let signature = map_path.get_base64("signature")?;

    let timestamp = map_path.get_num("timestamp")?;
    let message_id = map_path.get_uint256("message_id")?;

    let status = match map_path.get_str("kind")? {
        // RempMessageStatus::TonNode_RempAccepted
        s @ ("IncludedIntoBlock" | "AcceptedByFullnode" | "Finalized" | "AcceptedByQueue" | "IncludedIntoAcceptedBlock") => {
            let level = match s {
                "IncludedIntoBlock" => RempMessageLevel::TonNode_RempCollator,
                "AcceptedByFullnode" => RempMessageLevel::TonNode_RempFullnode,
                "Finalized" => RempMessageLevel::TonNode_RempMasterchain,
                "AcceptedByQueue" => RempMessageLevel::TonNode_RempQueue,
                "IncludedIntoAcceptedBlock" => RempMessageLevel::TonNode_RempShardchain,
                s => fail!("Unknown status: {}", s)
            };
            RempMessageStatus::TonNode_RempAccepted (
                rempmessagestatus::RempAccepted {
                    level,
                    block_id: parse_block_id_ext(&map_path, false)?,
                    master_id: parse_block_id_ext(&map_path, true).unwrap_or_default(),
                }
            )
        }
        "Duplicate" => {
            RempMessageStatus::TonNode_RempDuplicate (
                rempmessagestatus::RempDuplicate {
                    block_id: parse_block_id_ext(&map_path, false)?,
                }
            )
        }
        s @ ("IgnoredByCollator" | "IgnoredByFullNode" | "IgnoredByMasterchain" | "IgnoredByQueue" | "IgnoredByShardchain") => {
            let level = match s {
                "IgnoredByCollator" => RempMessageLevel::TonNode_RempCollator,
                "IgnoredByFullNode" => RempMessageLevel::TonNode_RempFullnode,
                "IgnoredByMasterchain" => RempMessageLevel::TonNode_RempMasterchain,
                "IgnoredByQueue" => RempMessageLevel::TonNode_RempQueue,
                "IgnoredByShardchain" => RempMessageLevel::TonNode_RempShardchain,
                s => fail!("Unknown status: {}", s)
            };
            RempMessageStatus::TonNode_RempIgnored (
                rempmessagestatus::RempIgnored {
                    level,
                    block_id: parse_block_id_ext(&map_path, false)?,
                }
            )
        }
        // RempMessageStatus::TonNode_RempNew
        "PutIntoQueue" => {
            RempMessageStatus::TonNode_RempNew
        }
        // RempMessageStatus::TonNode_RempRejected
        s @ ("RejectedByCollator" | "RejectedByFullnode" | "RejectedByMasterchain" | "RejectedByQueue" | "RejectedByShardchain") => {
            let level = match s {
                "RejectedByCollator" => RempMessageLevel::TonNode_RempCollator,
                "RejectedByFullnode" => RempMessageLevel::TonNode_RempFullnode,
                "RejectedByMasterchain" => RempMessageLevel::TonNode_RempMasterchain,
                "RejectedByQueue" => RempMessageLevel::TonNode_RempQueue,
                "RejectedByShardchain" => RempMessageLevel::TonNode_RempShardchain,
                s => fail!("Unknown status: {}", s)
            };
            RempMessageStatus::TonNode_RempRejected (
                rempmessagestatus::RempRejected {
                    level,
                    block_id: parse_block_id_ext(&map_path, false)?,
                    error: map_path.get_str("error")?.into(),
                }
            )
        }
        // RempMessageStatus::TonNode_RempSentToValidators
        "SentToValidators" => {
            RempMessageStatus::TonNode_RempSentToValidators (
                rempmessagestatus::RempSentToValidators {
                    sent_to: map_path.get_num("sent_to")? as i32,
                    total_validators: map_path.get_num("total_validators")? as i32,
                }
            )
        }
        // RempMessageStatus::TonNode_RempTimeout
        "Timeout" => {
            RempMessageStatus::TonNode_RempTimeout
        }
        s => fail!("Unknown status: {}", s)
    };

    let receipt = ton_api::ton::ton_node::rempreceipt::RempReceipt {
        message_id,
        status,
        timestamp,
        source_id,
    }.into_boxed();

    Ok((receipt, signature))
}

pub fn parse_block_proof(
    map: &Map<String, Value>, block_file_hash: UInt256
) -> Result<ton_dev_block::BlockProof> {

    let map_path = PathMap::new(map);

    let root = ton_dev_block::read_single_root_boc(base64_decode(map_path.get_str("proof")?)?)?;

    let merkle_proof = ton_dev_block::MerkleProof::construct_from_cell(root.clone())?;
    let block_virt_root = merkle_proof.proof.virtualize(1);
    let virt_block = ton_dev_block::Block::construct_from_cell(block_virt_root.clone())?;
    let block_info = virt_block.read_info()?;

    let proof_for = BlockIdExt::with_params(
        ton_dev_block::ShardIdent::with_tagged_prefix(
            block_info.shard().workchain_id(),
            block_info.shard().shard_prefix_with_tag(),
        )?,
        block_info.seq_no(),
        block_virt_root.repr_hash(),
        block_file_hash,
    );

    let signatures = if let Ok(signatures) = map_path.get_vec("signatures") {
        let mut pure_signatures = ton_dev_block::BlockSignaturesPure::new();
        pure_signatures.set_weight(map_path.get_num("sig_weight")? as u64);
        for signature in signatures {
            let signature = PathMap::cont(&map_path, "signatures", signature)?;
            pure_signatures.add_sigpair(ton_dev_block::CryptoSignaturePair {
                node_id_short: signature.get_uint256("node_id")?,
                sign: ton_dev_block::CryptoSignature::from_r_s_str(
                    signature.get_str("r")?,
                    signature.get_str("s")?,
                )?
            });
        }
        Some(ton_dev_block::BlockSignatures::with_params(
            ton_dev_block::ValidatorBaseInfo::with_params(
                map_path.get_num("validator_list_hash_short")? as u32,
                map_path.get_num("catchain_seqno")? as u32,
            ),
            pure_signatures
        ))
    } else {
        None
    };

    Ok(ton_dev_block::BlockProof::with_params(proof_for, root, signatures))
}

#[cfg(test)]
#[path = "tests/test_deserialize.rs"]
mod tests;
