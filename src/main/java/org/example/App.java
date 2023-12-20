package org.example;

import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubIO;
import org.apache.beam.sdk.options.PipelineOptionsFactory;
import org.apache.beam.sdk.options.PipelineOptions;
import org.apache.beam.sdk.transforms.DoFn;
import org.apache.beam.sdk.transforms.ParDo;
import org.apache.beam.sdk.transforms.windowing.FixedWindows;
import org.apache.beam.sdk.transforms.windowing.Window;
import org.joda.time.Duration;

public class App {
    public static void main(String[] args) {
        System.out.println("stuff");
        int numShards = 1;
        int windowSize = 2;
        String outputFilePath = "gs://tmp_bucket_93/temp";
        String topicName = "projects/neon-circle-400322/topics/topic-sub-1";

        PipelineOptions options = PipelineOptionsFactory.create();
        Pipeline pipeline = Pipeline.create(options);

        pipeline
                .apply("Read PubSub Messages", PubsubIO.readStrings().fromTopic(topicName))
                .apply("windowing", Window.into(FixedWindows.of(Duration.standardMinutes(windowSize))))
                .apply("logging", ParDo.of(new MyT()));

        pipeline.run().waitUntilFinish();

    }
}

class MyT extends DoFn<String, String> {
    @ProcessElement
    public void processElement(@Element String word, OutputReceiver<String> out) {
        System.out.println(word);
        out.output(word);
    }
}