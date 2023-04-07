use enumset::EnumSet;

use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::model::var::Objid;

pub trait Permissions {
    fn property_allows(
        &mut self,
        check_flags: EnumSet<PropFlag>,
        player: Objid,
        player_flags: EnumSet<ObjFlag>,
        prop_flags: EnumSet<PropFlag>,
        prop_owner: Objid,
    ) -> bool;
}
