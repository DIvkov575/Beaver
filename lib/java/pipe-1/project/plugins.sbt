// Copyright 2022 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

resolvers += Resolver.jcenterRepo

// Assemble self-contained jar files.
addSbtPlugin("com.eed3si9n" % "sbt-assembly" % "1.1.0")

// JUnit 5 testing integration.
addSbtPlugin("net.aichler" % "sbt-jupiter-interface" % "0.9.0")
