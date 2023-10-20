resolvers += Resolver.jcenterRepo

// Assemble self-contained jar files.
addSbtPlugin("com.eed3si9n" % "sbt-assembly" % "1.1.0")

// JUnit 5 testing integration.
addSbtPlugin("net.aichler" % "sbt-jupiter-interface" % "0.9.0")
