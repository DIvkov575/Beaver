

create dir w examples:
mvn archetype:generate \
-DarchetypeGroupId=org.apache.beam \
-DarchetypeArtifactId=beam-sdks-java-maven-archetypes-examples \
-DarchetypeVersion=2.52.0 \
-DgroupId=org.example \
-DartifactId=word-count-beam \
-Dversion="0.1" \
-Dpackage=org.apache.beam.examples \
-DinteractiveMode=false


run:
mvn compile exec:java -Dexec.mainClass=org.apache.beam.examples.WordCount \
-Dexec.args="--inputFile=sample.txt --output=counts" -Pdirect-runner


Create bucket:
    gsutil mb gs://$BUCKET_NAME

Create pubsub topic:
    gcloud pubsub topics create $TOPIC_ID


publishes messages to pubsub once a minute:
    gcloud scheduler jobs create pubsub publisher-job --schedule="* * * * *" \
    --topic=$TOPIC_ID --message-body="Hello!" --location=$REGION

    gcloud scheduler jobs run publisher-job --location=$REGION

