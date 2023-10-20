mainClass := Some("com.example.App")

val beamVersion = "2.46.0"
libraryDependencies ++= Seq(
  "org.apache.beam" % "beam-sdks-java-core" % beamVersion,
  "org.apache.beam" % "beam-runners-direct-java" % beamVersion,
  "org.slf4j" % "slf4j-jdk14" % "1.7.32",

  // Test dependencies.
  "junit" % "junit" % "4.13.2" % Test,
  "com.novocode" % "junit-interface" % "0.11" % Test,
  "org.hamcrest" % "hamcrest" % "2.2" % Test
)

// Package self-contained jar file.
assembly / assemblyOutputPath := file("build/pipeline.jar")
assembly / assemblyMergeStrategy := {
  case PathList("META-INF")      => MergeStrategy.discard
  case x if x.endsWith(".class") => MergeStrategy.first
  case x                         => (assembly / assemblyMergeStrategy).value(x)
}
