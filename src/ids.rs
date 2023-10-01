macro_rules! ids {
    ($(- uuid: $uuid:literal name: $name:literal id: org.bluetooth.service. $id:ident)*) => {
        $(
            #[doc = stringify!($name)]
            #[doc = " (org.bluetooth.service."]
            #[doc = stringify!($id)]
            #[doc = ")"]
            #[allow(non_upper_case_globals)]
            pub const $id: uuid::Uuid = uuid::Uuid::from_fields($uuid, 0x0, 0x1000, &[0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb]);
        )*
    };
    ($(- uuid: $uuid:literal name: $name:literal id: org.bluetooth.characteristic. $id:ident)*) => {
        $(
            #[doc = stringify!($name)]
            #[doc = " (org.bluetooth.characteristic."]
            #[doc = stringify!($id)]
            #[doc = ")"]
            #[allow(non_upper_case_globals)]
            pub const $id: uuid::Uuid = uuid::Uuid::from_fields($uuid, 0x0, 0x1000, &[0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb]);
        )*
    };
}

pub mod service {
    ids! {
        - uuid: 0x1825
          name: "Object Transfer"
          id: org.bluetooth.service.object_transfer
    }
}

pub mod characteristic {
    ids! {
        - uuid: 0x2ABD
          name: "OTS Feature"
          id: org.bluetooth.characteristic.ots_feature
        - uuid: 0x2ABE
          name: "Object Name"
          id: org.bluetooth.characteristic.object_name
        - uuid: 0x2ABF
          name: "Object Type"
          id: org.bluetooth.characteristic.object_type
        - uuid: 0x2AC0
          name: "Object Size"
          id: org.bluetooth.characteristic.object_size
        - uuid: 0x2AC1
          name: "Object First-Created"
          id: org.bluetooth.characteristic.object_first_created
        - uuid: 0x2AC2
          name: "Object Last-Modified"
          id: org.bluetooth.characteristic.object_last_modified
        - uuid: 0x2AC3
          name: "Object ID"
          id: org.bluetooth.characteristic.object_id
        - uuid: 0x2AC4
          name: "Object Properties"
          id: org.bluetooth.characteristic.object_properties
        - uuid: 0x2AC5
          name: "Object Action Control Point"
          id: org.bluetooth.characteristic.object_action_control_point
        - uuid: 0x2AC6
          name: "Object List Control Point"
          id: org.bluetooth.characteristic.object_list_control_point
        - uuid: 0x2AC7
          name: "Object List Filter"
          id: org.bluetooth.characteristic.object_list_filter
        - uuid: 0x2AC8
          name: "Object Changed"
          id: org.bluetooth.characteristic.object_changed
    }
}
