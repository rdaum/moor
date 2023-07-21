use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::util::bitenum::BitEnum;
use crate::var::Objid;

pub trait Permissions {
    fn property_allows(
        &mut self,
        check_flags: BitEnum<PropFlag>,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        prop_flags: BitEnum<PropFlag>,
        prop_owner: Objid,
    ) -> bool;
}
