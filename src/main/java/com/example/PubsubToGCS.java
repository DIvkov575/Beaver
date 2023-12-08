package com.example;
import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubIO;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubMessage;
import org.apache.beam.sdk.io.TextIO;
import org.apache.beam.sdk.options.PipelineOptions;
import org.apache.beam.sdk.options.PipelineOptionsFactory;
import org.apache.beam.sdk.transforms.MapElements;
import org.apache.beam.sdk.transforms.SimpleFunction;

public class PubsubToGCS {

    public static void main(String[] args) {
        // Set your GCP project, Pub/Sub topic, and GCS output path
        String projectId = "neon-circle-400322";
        String pubsubTopic = "projects/neon-circle-400322/topics/topic-sub-1";
        String gcsOutputPath = "gs://tmp_bucket_93/output1";
        PipelineOptions options = PipelineOptionsFactory.fromArgs(args).create();

        Pipeline pipeline = Pipeline.create(options);
        pipeline
                .apply("ReadFromPubSub", PubsubIO.readMessagesWithAttributes().fromTopic("projects/" + projectId + "/topics/" + pubsubTopic))
                .apply("ConvertToString", MapElements
                        .<PubsubMessage, String>via(new PubsubMessageToString()))

                .apply("WriteToGCS", TextIO.write().to(gcsOutputPath).withoutSharding());

        pipeline.run();
    }

    private static class PubsubMessageToString extends SimpleFunction<PubsubMessage, String> {
        @Override
        public String apply(PubsubMessage message) {
            return new String(message.getPayload());
        }
    }
}
