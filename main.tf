provider "google" {
  project     = "neon-circle-400322"
  region      = "us-central1"
}

resource "google_bigquery_dataset" "data_lake" {
  dataset_id = "data_lake"
}

# Define sources and pipelines
variable "data_sources" {
  type = list(object({
    name        = string
    source_type = string
    location    = string
  }))
  default = [
    {
      name        = "source_1"
      source_type = "cloud_storage"
      location    = "gs://source_1_bucket"
    },
    {
      name        = "source_2"
      source_type = "bigtable"
      location    = "projects/project-id/instances/instance-id/tables/table-id"
    },
  ]
}

# data sources
resource "google_bigquery_external_table" "external_tables" {
  count       = length(var.data_sources)
  dataset_id  = google_bigquery_dataset.data_lake.dataset_id
  table_id    = var.data_sources[count.index].name
  source_type = var.data_sources[count.index].source_type

  external_data_configuration {
    source_uris = [var.data_sources[count.index].location]
    autodetect = true
  }
}

# pipeline
resource "google_dataflow_job" "dataflow_pipelines" {
  count     = length(var.data_sources)
  name      = "pipeline_${var.data_sources[count.index].name}"
  project   = var.project
  region    = var.region
  # template_gcs_path = "gs://dataflow-templates/latest/GCS_Text_to_BigQuery"
  template_gcs_path = "gs://my-template-bucket/template
  
  parameters = {
    inputFilePattern = var.data_sources[count.index].location
    outputTableSpec  = "${google_bigquery_dataset.data_lake.dataset_id}.${var.data_sources[count.index].name}"
  }
}
