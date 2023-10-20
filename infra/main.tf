locals {
  region           = var.region
  project          = var.project
  container_image1 = var.container_image1
  jobs             = {
    max_workers = 2
  }
}

provider "google" {
  project = local.project
  region  = local.region
}

# sources

resource "google_logging_project_sink" "logging_sink" {
  count       = length(var.sources)
  name        = "sink-${var.sources[count.index]}"
  project     = var.project
#  filter      = "LOG_FILTER"  # Define your log filter
  destination = "pubsub.googleapis.com/projects/${var.project}/topics/${google_pubsub_topic.pubsub_topic[count.index].name}"
}

# pipeline

resource "google_pubsub_topic" "pubsub_topic" {
  count = length(var.sources)
  name  = "topic-${var.sources[count.index]}"
}

#resource "google_pubsub_subscription" "pubsub_subscription" {
#  count = length(var.sources)
#  name  = "subscription-${var.sources[count.index]}"
#  topic = google_pubsub_topic.pubsub_topic[count.index].name
#}

#resource "google_dataflow_job" "dataflow_job" {
#  count = length(var.sources)
#  name         = "dataflow-job-${var.sources[count.index]}"
#  project      = var.project
#  region       = var.region
#  temp_gcs_location = google_storage_bucket.temporary_storage_bucket.location
#  template_gcs_path = "gs://dataflow-templates/pubsub-to-bigquery/streaming-autogenerated-json/v1-1"
#
#  parameters = {
#    inputTopic  = google_pubsub_subscription.pubsub_subscription[count.index].name
#    outputTable = "${var.project}:${google_bigquery_dataset.bigquery_dataset.dataset_id}.${google_bigquery_table.bigquery_table.table_id}"
#  }
#}

# storage

#resource "google_bigquery_dataset" "bigquery_dataset" {
#  dataset_id = "bigquery_dataset_1"
#  project    = var.project
#}

#resource "google_bigquery_table" "bigquery_table" {
#  dataset_id = google_bigquery_dataset.bigquery_dataset.dataset_id
#  project    = var.project
#  table_id   = "bigquery_table_1"
#}

#resource "google_storage_bucket" "temporary_storage_bucket" {
#  name     = "tmp_bucket_1923874"
#  location = local.region
#  storage_class = "REGIONAL"
#  labels = {
#    purpose = "temporary-storage"
#  }
#}