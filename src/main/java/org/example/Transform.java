package org.example;
import org.apache.beam.sdk.transforms.DoFn;

public class Transform extends DoFn<String, String> {
    @ProcessElement
    public void processElement(@Element String word, OutputReceiver<String> out) {
        

        out.output(word);
    }
}

