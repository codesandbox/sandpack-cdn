@0xc21d4fcf7858539c;

# https://capnproto.org/language.html
struct DistTag {
    tag @0 :Text;
    version @1 :Text;
}

struct PackageDependency {
    name @0 :Text;
    version @1 :Text;
}

struct PackageVersion {
    version @0 :Text;
    tarball @1 :Text;
    dependencies @2 :List(PackageDependency);
}

struct MinimalPackageData {
    name @0 :Text;
    distTags @1 :List(DistTag);
    versions @2 :List(PackageVersion);
}
