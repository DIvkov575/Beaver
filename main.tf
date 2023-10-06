locals {
  region = "us-central1"
  project = "neon-circle-400322"
}

provider "google" {
  project = local.project
  region = local.region
}

resource "google_bigquery_connection" "connection" {
  connection_id = "CONNECTION_ID_1"
  project = local.project
  location = local.region
  cloud_resource {}
}        

resource "google_project_iam_member" "connectionPermissionGrant" {
  project = local.project
  role = "roles/storage.objectViewer"
  member = format("serviceAccount:%s", google_bigquery_connection.connection.cloud_resource[0].service_account_id)
}  