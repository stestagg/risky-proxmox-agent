#include <ctype.h>
#include <curl/curl.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define API_BASE_URL "http://10.0.2.2:3000"
#define MAX_VMS 256
#define NAME_SIZE 128
#define STATUS_SIZE 32

struct vm_record {
    unsigned long vmid;
    char name[NAME_SIZE];
    char status[STATUS_SIZE];
};

struct response_buffer {
    char *data;
    size_t len;
};

static size_t append_response(void *contents, size_t size, size_t nmemb, void *userdata) {
    size_t add_len = size * nmemb;
    struct response_buffer *buffer = (struct response_buffer *)userdata;
    char *new_data = realloc(buffer->data, buffer->len + add_len + 1);
    if (new_data == NULL) {
        return 0;
    }

    buffer->data = new_data;
    memcpy(buffer->data + buffer->len, contents, add_len);
    buffer->len += add_len;
    buffer->data[buffer->len] = '\0';
    return add_len;
}

static int http_request(const char *method, const char *path, const char *body, struct response_buffer *response) {
    CURL *curl = curl_easy_init();
    if (curl == NULL) {
        return -1;
    }

    char url[512];
    snprintf(url, sizeof(url), "%s%s", API_BASE_URL, path);

    response->data = malloc(1);
    response->len = 0;
    if (response->data == NULL) {
        curl_easy_cleanup(curl);
        return -1;
    }
    response->data[0] = '\0';

    struct curl_slist *headers = NULL;
    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 10L);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, append_response);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, response);

    if (strcmp(method, "POST") == 0) {
        headers = curl_slist_append(headers, "Content-Type: application/json");
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(curl, CURLOPT_POST, 1L);
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body == NULL ? "{}" : body);
    }

    CURLcode rc = curl_easy_perform(curl);
    long status_code = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &status_code);

    if (headers != NULL) {
        curl_slist_free_all(headers);
    }
    curl_easy_cleanup(curl);

    if (rc != CURLE_OK || status_code >= 400) {
        return -1;
    }

    return 0;
}

static void trim_copy(char *dst, size_t dst_size, const char *src_start, size_t src_len) {
    while (src_len > 0 && isspace((unsigned char)*src_start)) {
        src_start++;
        src_len--;
    }
    while (src_len > 0 && isspace((unsigned char)src_start[src_len - 1])) {
        src_len--;
    }

    size_t copy_len = src_len < (dst_size - 1) ? src_len : (dst_size - 1);
    memcpy(dst, src_start, copy_len);
    dst[copy_len] = '\0';
}

static bool extract_json_string(const char *obj, const char *key, char *out, size_t out_size) {
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\":\"", key);
    const char *start = strstr(obj, pattern);
    if (start == NULL) {
        return false;
    }

    start += strlen(pattern);
    const char *end = strchr(start, '\"');
    if (end == NULL) {
        return false;
    }

    trim_copy(out, out_size, start, (size_t)(end - start));
    return true;
}

static bool extract_json_ulong(const char *obj, const char *key, unsigned long *out) {
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\":", key);
    const char *start = strstr(obj, pattern);
    if (start == NULL) {
        return false;
    }

    start += strlen(pattern);
    while (*start && isspace((unsigned char)*start)) {
        start++;
    }

    char *end = NULL;
    unsigned long value = strtoul(start, &end, 10);
    if (end == start) {
        return false;
    }

    *out = value;
    return true;
}

static size_t parse_vm_inventory(const char *json, struct vm_record *vms, size_t max_vms) {
    size_t count = 0;
    const char *cursor = json;

    while (*cursor != '\0' && count < max_vms) {
        const char *obj_start = strchr(cursor, '{');
        if (obj_start == NULL) {
            break;
        }

        const char *obj_end = strchr(obj_start, '}');
        if (obj_end == NULL) {
            break;
        }

        size_t obj_len = (size_t)(obj_end - obj_start + 1);
        char object[1024];
        size_t copy_len = obj_len < sizeof(object) - 1 ? obj_len : sizeof(object) - 1;
        memcpy(object, obj_start, copy_len);
        object[copy_len] = '\0';

        struct vm_record vm = {.vmid = 0};
        strcpy(vm.name, "Unnamed");
        strcpy(vm.status, "unknown");

        if (extract_json_ulong(object, "vmid", &vm.vmid)) {
            extract_json_string(object, "name", vm.name, sizeof(vm.name));
            extract_json_string(object, "status", vm.status, sizeof(vm.status));
            vms[count++] = vm;
        }

        cursor = obj_end + 1;
    }

    return count;
}

static bool response_has_needs_action(const char *json) {
    return strstr(json, "\"status\":\"needs_action\"") != NULL;
}

static int launch_vm(unsigned long vmid) {
    char payload[128];
    snprintf(payload, sizeof(payload), "{\"vmid\":%lu}", vmid);

    struct response_buffer response = {0};
    if (http_request("POST", "/api/launch", payload, &response) != 0) {
        free(response.data);
        return -1;
    }

    if (response_has_needs_action(response.data)) {
        char force_payload[192];
        snprintf(force_payload, sizeof(force_payload), "{\"vmid\":%lu,\"action\":\"terminate\"}", vmid);
        free(response.data);
        response.data = NULL;
        response.len = 0;

        if (http_request("POST", "/api/launch", force_payload, &response) != 0) {
            free(response.data);
            return -1;
        }
    }

    printf("Launch response: %s\n", response.data);
    free(response.data);
    return 0;
}

static int shutdown_host(void) {
    const char *payload = "{\"action\":\"terminate\"}";
    struct response_buffer response = {0};

    if (http_request("POST", "/api/host-shutdown", payload, &response) != 0) {
        free(response.data);
        return -1;
    }

    if (response_has_needs_action(response.data)) {
        free(response.data);
        response.data = NULL;
        response.len = 0;

        if (http_request("POST", "/api/host-shutdown", payload, &response) != 0) {
            free(response.data);
            return -1;
        }
    }

    printf("Shutdown response: %s\n", response.data);
    free(response.data);
    return 0;
}

int main(void) {
    printf("Risky Proxmox uKernel loader starting...\n");
    printf("API endpoint: %s\n\n", API_BASE_URL);

    curl_global_init(CURL_GLOBAL_DEFAULT);

    while (1) {
        struct response_buffer vm_response = {0};
        if (http_request("GET", "/api/vms", NULL, &vm_response) != 0) {
            printf("Unable to fetch VM list from %s/api/vms\n", API_BASE_URL);
            free(vm_response.data);
            break;
        }

        struct vm_record vms[MAX_VMS];
        size_t vm_count = parse_vm_inventory(vm_response.data, vms, MAX_VMS);
        free(vm_response.data);

        printf("Available VMs (%zu):\n", vm_count);
        for (size_t i = 0; i < vm_count; i++) {
            printf("  %zu) %s (#%lu) - %s\n", i + 1, vms[i].name, vms[i].vmid, vms[i].status);
        }

        printf("\nChoose: [number] launch VM, [r]efresh, [s]hutdown host, [q]uit > ");
        char input[32];
        if (fgets(input, sizeof(input), stdin) == NULL) {
            break;
        }

        if (input[0] == 'q') {
            break;
        }
        if (input[0] == 'r') {
            printf("\n");
            continue;
        }
        if (input[0] == 's') {
            if (shutdown_host() != 0) {
                printf("Host shutdown request failed.\n");
            }
            printf("\n");
            continue;
        }

        long chosen = strtol(input, NULL, 10);
        if (chosen <= 0 || (size_t)chosen > vm_count) {
            printf("Invalid selection.\n\n");
            continue;
        }

        unsigned long vmid = vms[chosen - 1].vmid;
        if (launch_vm(vmid) != 0) {
            printf("Launch request failed for VM %lu\n", vmid);
        }
        printf("\n");
    }

    curl_global_cleanup();
    printf("Exiting uKernel loader.\n");
    return 0;
}
