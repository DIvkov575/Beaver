package org.example;

//import com.google.api.services.bigquery.model.TableRow;
import com.google.api.services.bigquery.model.*;

import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.util.AbstractMap.SimpleEntry;
import java.util.Arrays;
import java.util.Base64;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.apache.beam.sdk.transforms.DoFn;

public class WriteToTable extends DoFn<String, String> {
    @ProcessElement
    public void processElement(@Element String word, OutputReceiver<String> out) {

        TableReference tableSpec =
                new TableReference()
                        .setProjectId("clouddataflow-readonly")
                        .setDatasetId("samples")
                        .setTableId("weather_stations");

        out.output(word);
    }
}


class BigQueryTableRowCreate {
    public static TableRow createTableRow() {
        TableRow row =
                new TableRow()
                        .set("string_field", "UTF-8 strings are supported! 🌱🌳🌍")
                        .set("timestamp_field", Instant.parse("2020-03-20T03:41:42.123Z").toString());
        return row;
    }
}