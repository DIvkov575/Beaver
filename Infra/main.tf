locals {
  region           = var.region
  project          = var.region
  container_image1 = var.container_image1
}

provider "google" {
  project = local.project
  region  = local.region
}

resource "google_dataflow_job" "log_sink_to_bigquery_job" {
  name       = "log-sink-to-bigquery-job"
  template_gcs_path = "gs://my-bucket/templates/template_file"
  temp_gcs_location = "gs://my-bucket/tmp_dir"
  parameters = {
    input_log_sink        = "log_sink"
    output_bigquery_table = google_bigquery_table.table1.id
    container_image       = var.container_image1
  }
}

resource "google_logging_project_sink" "log_sink" {
  name        = "my-sink"
  destination = "storage.googleapis.com/my-project/my-bucket"
}

resource "google_bigquery_dataset" "dataset" {
  dataset_id                  = "your_dataset_id"
  project                     = local.project
  location                    = "US"  # Set your desired location
  default_table_expiration_ms = 3600000  # Set the expiration time for tables in milliseconds (optional)
}

resource "google_bigquery_table" "table1" {
  dataset_id = google_bigquery_dataset.dataset.dataset_id
  table_id   = "your_table_id"
}

