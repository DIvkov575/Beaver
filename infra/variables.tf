variable "region" {
  type = string
  default = "us-central1"
}

variable "project" {
  type = string
  default = "neon-circle-400322"
}

variable "container_image1" {
  type = string
  default = "timberio/vector:0.33.0-debian"
}

variable "pubsub_topic" {
  description = "Pub/Sub topic name"
  default = "pubsub-topic"
}

variable "sources" {
  default = ["sub-1"]
}

