import java.io.IOException;
import java.util.logging.Level;
import java.util.logging.Logger;
import java.util.logging.*;

import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubIO;
import org.apache.beam.sdk.options.Default;
import org.apache.beam.sdk.options.Description;
import org.apache.beam.sdk.options.PipelineOptionsFactory;
import org.apache.beam.sdk.options.PipelineOptions;
import org.apache.beam.sdk.options.StreamingOptions;
import org.apache.beam.sdk.options.Validation.Required;
import org.apache.beam.sdk.transforms.windowing.FixedWindows;
import org.apache.beam.sdk.transforms.windowing.Window;
import org.joda.time.Duration;

//import org.apache.beam.examples.other;

public class Other {
    private final static Logger LOGGER = Logger.getLogger(Logger.GLOBAL_LOGGER_NAME);
    public static void main(String[] args) {
        LOGGER.log(Level.INFO, "My first Log Message");


        int numShards = 1;
        String outputFilePath = "gs://tmp_bucket_93/temp";
        int windowSize = 2;
        String topicName = "projects/neon-circle400322/topics/topic-sub-1";


        PipelineOptions options = PipelineOptionsFactory.create();
        Pipeline pipeline = Pipeline.create(options);

        pipeline
        .apply("Read PubSub Messages", PubsubIO.readStrings().fromTopic(topicName))
        .apply("windowing", Window.into(FixedWindows.of(Duration.standardMinutes(windowSize))));
//        .apply("Write Files to GCS", new WriteOneFilePerWindow(outputFilePath, numShards));

        //      PCollection<PubsubMessage> message =  pipeline.apply("Read", PubsubIO.readMessagesWithAttributes().fromSubscription('subscription_name'));

        pipeline.run().waitUntilFinish();

  }

}
//    --project=$PROJECT_ID \
//        --region=$REGION \
//        --inputTopic=projects/$PROJECT_ID/topics/$TOPIC_ID \
//        --output=gs://$BUCKET_NAME/samples/output \
//        --gcpTempLocation=gs://$BUCKET_NAME/temp \
//        --runner=DataflowRunner \
//        --windowSize=2 \
//        --serviceAccount=$SERVICE_ACCOUNT"
