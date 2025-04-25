# 🦫 Beaver SIEM

*Secure. Analyze. Monitor.*

Beaver SIEM is a cutting-edge **data security log analysis tool** designed to protect your infrastructure by monitoring and analyzing logs in real-time. It ensures your system’s safety by helping identify potential security breaches and threats through advanced log parsing and analysis techniques.

---

## 🚀 **Build**

To get started with Beaver SIEM, you will need to install and build it using the standard tools. Make sure to set up the required environment before proceeding.
```
Cargo install --release --path .
```
⚙️ *This step will build Beaver SIEM in release mode. Reload shell after install*

---

## 🛠️ **Setup**

After building, you can initialize Beaver SIEM through a simple setup process that creates the necessary configurations and sets up your environment.
```
Beaver init
```
⚙️ *Ensure that your system has the required permissions and environment variables configured.*


---

### 📋 **Current Todo:**

> The following tasks are in progress or planned for the next release:

- **l_0** - crj shutoff - missing `{}` as second arg in the last `.get()` chain method call
- **l_1** - Add SA delegation - destructured log -> BQ - logging

---

### 💡 **Ideas for Future Enhancements:**

- Disable `detections_gen.py` regeneration to streamline the development process.
- Implement batching to improve efficiency in BigQuery by deduplicating writes after log destructuring.
  
    📝 *This will help reduce the amount of redundant data being written, enhancing write efficiency.*

---

### ⏳ **Current Progress:**

- Create Cloud Run jobs.
- Set up BigQuery.
- Create Pub/Sub topic **(1)** and subscription **(2)** for BigQuery integration.
- Provision storage bucket for log storage.
- Link Cloud Run jobs to storage bucket for secure storage and retrieval.

---


> “Stay secure, stay informed.”



### **Common Issues **
- Initialization failed 
  ```        
  ERROR: Failed building wheel for grpcio-tools
      Failed to build grpcio-tools
      ERROR: Failed to build installable wheels for some pyproject.toml based projects (grpcio-tools)
      [end of output]
  
  note: This error originates from a subprocess, and is likely not a problem with pip.
  error: subprocess-exited-with-error
  ```
  Source: Apache beam has limited python 3.x support
  Solution: Check currently supported python versions (3.8, 3.9, 3.10, 3.11 as of 4/22/25)

[//]: # (- `apitools` auth on dataflow creation)
[//]: # (  Source: apache_beam[gcs] client library needs authentication)
[//]: # (  Solution: gcloud auth application-default login)



### **personal notes**
 - Not CRS bc
