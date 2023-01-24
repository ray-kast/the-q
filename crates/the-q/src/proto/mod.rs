macro_rules! proto_mod {
    ($vis:vis $name:ident, $package:literal) => {
        $vis mod $name {
            include!(concat!(env!("OUT_DIR"), "/", $package, ".rs"));
        }
    };
}

proto_mod!(pub modal, "modal");
proto_mod!(pub component, "component");
