use classic_terraswap::asset::PairInfoRaw;
use cw_storage_plus::Item;

pub const PAIR_INFO: Item<PairInfoRaw> = Item::new("\u{0}\u{9}pair_info");
