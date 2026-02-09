#include <Alert.h>
#include <Application.h>
#include <Button.h>
#include <LayoutBuilder.h>
#include <ListView.h>
#include <ScrollView.h>
#include <StringItem.h>
#include <StringView.h>
#include <TextControl.h>
#include <Window.h>

#include <cstdio>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <memory>
#include <regex>
#include <sstream>
#include <string>
#include <vector>

namespace {

const char* kAppSignature = "application/x-vnd.risky-proxmox-haiku-client";

const uint32 kRefreshMessage = 'refr';
const uint32 kLaunchMessage = 'lnch';

struct VmRecord {
    uint64 vmid;
    std::string name;
    std::string status;
};

std::string Trim(const std::string& input)
{
    const auto start = input.find_first_not_of(" \t\n\r");
    if (start == std::string::npos) {
        return "";
    }

    const auto end = input.find_last_not_of(" \t\n\r");
    return input.substr(start, end - start + 1);
}

std::string ExecuteCommand(const std::string& command)
{
    std::array<char, 512> buffer {};
    std::string output;

    FILE* pipe = popen(command.c_str(), "r");
    if (pipe == nullptr) {
        return output;
    }

    while (fgets(buffer.data(), buffer.size(), pipe) != nullptr) {
        output += buffer.data();
    }

    pclose(pipe);
    return output;
}

std::string JsonValue(const std::string& object, const std::string& key)
{
    const std::regex pattern("\\\"" + key + "\\\"\\s*:\\s*(\\\"([^\\\"]*)\\\"|[0-9]+|null)");
    std::smatch match;

    if (!std::regex_search(object, match, pattern)) {
        return "";
    }

    if (match.size() >= 3 && match[2].matched) {
        return match[2].str();
    }

    return match[1].str();
}

std::vector<VmRecord> ParseVms(const std::string& json)
{
    std::vector<VmRecord> vms;
    const std::regex objectRegex("\\{[^\\{\\}]*\\}");

    auto begin = std::sregex_iterator(json.begin(), json.end(), objectRegex);
    auto end = std::sregex_iterator();

    for (auto it = begin; it != end; ++it) {
        const std::string object = it->str();
        const std::string vmidString = JsonValue(object, "vmid");
        if (vmidString.empty()) {
            continue;
        }

        VmRecord vm {};
        vm.vmid = std::strtoull(vmidString.c_str(), nullptr, 10);
        vm.name = JsonValue(object, "name");
        vm.status = JsonValue(object, "status");
        if (vm.name.empty()) {
            vm.name = "Unnamed";
        }
        if (vm.status.empty()) {
            vm.status = "unknown";
        }
        vms.push_back(vm);
    }

    return vms;
}

std::string ApiRequest(const std::string& baseUrl, const std::string& path, const std::string& method = "GET",
    const std::string& payload = "")
{
    std::ostringstream command;
    command << "curl -s --max-time 10 ";
    if (method != "GET") {
        command << "-X " << method << " -H 'Content-Type: application/json' ";
    }

    if (!payload.empty()) {
        command << "-d '" << payload << "' ";
    }

    command << "'" << baseUrl << path << "'";
    return ExecuteCommand(command.str());
}

class MainWindow : public BWindow {
public:
    MainWindow()
        : BWindow(BRect(80, 80, 760, 520), "Risky Proxmox", B_TITLED_WINDOW, B_AUTO_UPDATE_SIZE_LIMITS)
        , fServerUrl(new BTextControl("Server:", "http://127.0.0.1:3000", nullptr))
        , fVmList(new BListView("vm-list", B_SINGLE_SELECTION_LIST))
        , fStatus(new BStringView("status", "Ready."))
        , fRefreshButton(new BButton("Refresh", new BMessage(kRefreshMessage)))
        , fLaunchButton(new BButton("Launch Selected VM", new BMessage(kLaunchMessage)))
    {
        fVmList->SetSelectionMessage(new BMessage(kLaunchMessage));

        BLayoutBuilder::Group<>(this, B_VERTICAL, B_USE_DEFAULT_SPACING)
            .SetInsets(B_USE_WINDOW_SPACING)
            .Add(fServerUrl)
            .AddGroup(B_HORIZONTAL, B_USE_DEFAULT_SPACING)
                .Add(fRefreshButton)
                .Add(fLaunchButton)
                .AddGlue()
            .End()
            .Add(new BScrollView("vm-scroll", fVmList, B_FRAME_EVENTS, false, true))
            .Add(fStatus);

        RefreshVmList();
    }

    bool QuitRequested() override
    {
        be_app->PostMessage(B_QUIT_REQUESTED);
        return true;
    }

    void MessageReceived(BMessage* message) override
    {
        switch (message->what) {
        case kRefreshMessage:
            RefreshVmList();
            return;
        case kLaunchMessage:
            if (message->HasInt32("index")) {
                return;
            }
            LaunchSelectedVm();
            return;
        default:
            BWindow::MessageReceived(message);
        }
    }

private:
    void SetStatus(const std::string& status)
    {
        fStatus->SetText(status.c_str());
    }

    std::string ServerUrl() const
    {
        return Trim(fServerUrl->Text());
    }

    void RefreshVmList()
    {
        const std::string url = ServerUrl();
        if (url.empty()) {
            SetStatus("Enter a server URL first.");
            return;
        }

        SetStatus("Loading VM inventory...");
        const std::string body = ApiRequest(url, "/api/vms");
        const auto vms = ParseVms(body);

        fVms = vms;
        fVmList->MakeEmpty();

        for (const auto& vm : fVms) {
            std::ostringstream line;
            line << vm.name << " (#" << vm.vmid << ") - " << vm.status;
            fVmList->AddItem(new BStringItem(line.str().c_str()));
        }

        std::ostringstream status;
        status << "Loaded " << fVms.size() << " VMs.";
        SetStatus(status.str());
    }

    void LaunchSelectedVm()
    {
        const int32 selected = fVmList->CurrentSelection();
        if (selected < 0 || static_cast<size_t>(selected) >= fVms.size()) {
            SetStatus("Select a VM first.");
            return;
        }

        const auto& vm = fVms[selected];
        const std::string url = ServerUrl();

        std::ostringstream payload;
        payload << "{\"vmid\":" << vm.vmid << "}";

        auto response = ApiRequest(url, "/api/launch", "POST", payload.str());

        const std::string status = JsonValue(response, "status");
        std::string message = JsonValue(response, "message");

        if (status == "needs_action") {
            const auto action = PromptForConflictAction();
            if (action == "cancel") {
                SetStatus("Launch cancelled.");
                return;
            }

            std::ostringstream actionPayload;
            actionPayload << "{\"vmid\":" << vm.vmid << ",\"action\":\"" << action << "\"}";
            response = ApiRequest(url, "/api/launch", "POST", actionPayload.str());
            message = JsonValue(response, "message");
        }

        if (message.empty()) {
            message = "Launch request submitted.";
        }

        SetStatus(message);
        RefreshVmList();
    }

    std::string PromptForConflictAction()
    {
        BAlert* alert = new BAlert(
            "action",
            "Another VM is running. Choose an action for the currently running VM:",
            "Shutdown",
            "Hibernate",
            "Terminate",
            B_WIDTH_FROM_LABEL,
            B_WARNING_ALERT);

        const int32 choice = alert->Go();
        switch (choice) {
        case 0:
            return "shutdown";
        case 1:
            return "hibernate";
        case 2:
            return "terminate";
        default:
            return "cancel";
        }
    }

    BTextControl* fServerUrl;
    BListView* fVmList;
    BStringView* fStatus;
    BButton* fRefreshButton;
    BButton* fLaunchButton;
    std::vector<VmRecord> fVms;
};

class RiskyProxmoxApp : public BApplication {
public:
    RiskyProxmoxApp()
        : BApplication(kAppSignature)
    {
    }

    void ReadyToRun() override
    {
        auto* window = new MainWindow();
        window->Show();
    }
};

} // namespace

int main()
{
    RiskyProxmoxApp app;
    app.Run();
    return 0;
}
