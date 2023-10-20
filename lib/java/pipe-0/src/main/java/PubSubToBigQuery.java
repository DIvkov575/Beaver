import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubIO;
import org.apache.beam.sdk.io.gcp.bigquery.BigQueryIO;
import org.apache.beam.sdk.options.*;
import org.apache.beam.sdk.transforms.MapElements;
import org.apache.beam.sdk.transforms.SimpleFunction;
import org.apache.beam.sdk.values.TypeDescriptor;

public class PubSubToBigQuery {

    public interface PubSubToBigQueryOptions extends PipelineOptions {
        @Description("Pub/Sub subscription path")
        @Validation.Required
        ValueProvider<String> getPubsubSubscription();

        void setPubsubSubscription(ValueProvider<String> value);

        @Description("BigQuery output table")
        @Validation.Required
        ValueProvider<String> getOutputTable();

        void setOutputTable(ValueProvider<String> value);
    }

    public static void main(String[] args) {
        PubSubToBigQueryOptions options = PipelineOptionsFactory.fromArgs(args)
                .withValidation()
                .as(PubSubToBigQueryOptions.class);

        Pipeline pipeline = Pipeline.create(options);

        pipeline
                .apply("ReadFromPubSub", PubsubIO.readStrings().fromSubscription(options.getPubsubSubscription()))
                .apply("TransformToTableRow", MapElements
                        .into(TypeDescriptor.of(String.class))
                        .via((String input) -> input)) // Modify this based on your data transformation
                .apply("WriteToBigQuery", BigQueryIO.writeTableRows() .to(options.getOutputTable()));
//                        .withSchema(...) // Define your BigQuery schema
//                        .withCreateDisposition(BigQueryIO.Write.CreateDisposition.CREATE_IF_NEEDED)
//                    .withwritedisposition(BigQueryIO.write.writedisposition.write_append));

        pipeline.run();
    }
}
