// Copyright 2022 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

package com.example;

import java.util.Arrays;
import java.util.List;

import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.coders.StringUtf8Coder;
import org.apache.beam.sdk.options.Default;
import org.apache.beam.sdk.options.Description;
import org.apache.beam.sdk.options.PipelineOptionsFactory;
import org.apache.beam.sdk.options.StreamingOptions;
import org.apache.beam.sdk.transforms.Create;
import org.apache.beam.sdk.transforms.MapElements;
import org.apache.beam.sdk.transforms.windowing.FixedWindows;
import org.apache.beam.sdk.transforms.windowing.Window;
import org.apache.beam.sdk.values.PCollection;
import org.apache.beam.sdk.values.TypeDescriptors;

import org.apache.beam.sdk.Pipeline;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubIO;
import org.apache.beam.sdk.io.gcp.pubsub.PubsubMessage;
import org.apache.beam.sdk.io.TextIO;
import org.apache.beam.sdk.options.PipelineOptions;
import org.apache.beam.sdk.options.PipelineOptionsFactory;
import org.apache.beam.sdk.transforms.MapElements;
import org.apache.beam.sdk.transforms.SimpleFunction;
import org.joda.time.Duration;

public class App {

	public static void main(String[] args) {
		String projectId = "neon-circle-400322";
		String pubsubTopic = "topic-sub-1";
//		String pubsubTopic = "projects/neon-circle-400322/topics/topic-sub-1";
//		String inputSubscription = "projects/neon-circle-400322/subscriptions/subscription-sub-1";
		String gcsOutputPath = "gs://tmp_bucket_93/output1";
		PipelineOptions options = PipelineOptionsFactory.fromArgs(args).create();

		Pipeline pipeline = Pipeline.create(options);
		pipeline
				.apply("ReadFromPubSub", PubsubIO.readMessagesWithAttributes().fromTopic("projects/" + projectId + "/topics/" + pubsubTopic))
				.apply("ConvertToString", MapElements.<PubsubMessage, String>via(new PubsubMessageToString()))
				.apply("Window", Window.<String>into(FixedWindows.of(Duration.standardSeconds(30))))
//				.apply("WriteToGCS", TextIO.write().to(gcsOutputPath).withNumShards(1));
				.apply("WriteToGCS", TextIO.write().to(gcsOutputPath).withWindowedWrites().withNumShards(1));

		pipeline.run();
	}

	private static class PubsubMessageToString extends SimpleFunction<PubsubMessage, String> {
		@Override
		public String apply(PubsubMessage message) {
			return new String(message.getPayload());
		}
	}
}
