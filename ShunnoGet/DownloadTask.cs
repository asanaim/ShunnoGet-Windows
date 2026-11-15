using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Text.Json;

namespace ShunnoGet;

public class DownloadItem : INotifyPropertyChanged
{
    private string _url = "";
    private string _fileName = "";
    private long _totalBytes = -1;
    private long _downloadedBytes = 0;
    private string _status = "QUEUED"; // QUEUED, DOWNLOADING, PAUSED, COMPLETED, FAILED
    private long _speedBytesPerSec = 0;
    private int _threadCount = 16;
    private string _category = "Documents";
    private bool _supportsRanges = true;
    private string? _referer;
    private string? _cookie;
    private string? _savedFilePath;
    private string? _sha256Hash;
    private string? _errorMessage;
    private string _segmentMapText = "";

    public long Id { get; set; } = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

    public string Url
    {
        get => _url;
        set { _url = value; OnPropertyChanged(); }
    }

    public string FileName
    {
        get => _fileName;
        set { _fileName = value; OnPropertyChanged(); }
    }

    public long TotalBytes
    {
        get => _totalBytes;
        set
        {
            _totalBytes = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(ProgressPercent));
            OnPropertyChanged(nameof(SizeProgressText));
            OnPropertyChanged(nameof(TelemetryText));
        }
    }

    public long DownloadedBytes
    {
        get => _downloadedBytes;
        set
        {
            _downloadedBytes = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(ProgressPercent));
            OnPropertyChanged(nameof(SizeProgressText));
            OnPropertyChanged(nameof(TelemetryText));
        }
    }

    public string Status
    {
        get => _status;
        set
        {
            _status = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(BadgeText));
            OnPropertyChanged(nameof(TelemetryText));
        }
    }

    public long SpeedBytesPerSec
    {
        get => _speedBytesPerSec;
        set
        {
            _speedBytesPerSec = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(TelemetryText));
        }
    }

    public int ThreadCount
    {
        get => _threadCount;
        set { _threadCount = value; OnPropertyChanged(); OnPropertyChanged(nameof(BadgeText)); }
    }

    public string Category
    {
        get => _category;
        set { _category = value; OnPropertyChanged(); OnPropertyChanged(nameof(BadgeText)); }
    }

    public bool SupportsRanges
    {
        get => _supportsRanges;
        set { _supportsRanges = value; OnPropertyChanged(); OnPropertyChanged(nameof(BadgeText)); }
    }

    public string? Referer
    {
        get => _referer;
        set { _referer = value; OnPropertyChanged(); }
    }

    public string? Cookie
    {
        get => _cookie;
        set { _cookie = value; OnPropertyChanged(); }
    }

    public string? SavedFilePath
    {
        get => _savedFilePath;
        set { _savedFilePath = value; OnPropertyChanged(); OnPropertyChanged(nameof(TelemetryText)); }
    }

    public string? Sha256Hash
    {
        get => _sha256Hash;
        set { _sha256Hash = value; OnPropertyChanged(); OnPropertyChanged(nameof(TelemetryText)); }
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        set { _errorMessage = value; OnPropertyChanged(); OnPropertyChanged(nameof(TelemetryText)); }
    }

    public string SegmentMapText
    {
        get => _segmentMapText;
        set { _segmentMapText = value; OnPropertyChanged(); }
    }

    public double ProgressPercent =>
        Status == "COMPLETED" ? 100.0 :
        TotalBytes > 0 ? Math.Clamp((DownloadedBytes * 100.0) / TotalBytes, 0.0, 100.0) : 0.0;

    public string BadgeText =>
        $"{Category} • {(SupportsRanges ? $"{ThreadCount} Threads" : "1 Stream")} • {Status}";

    public string SizeProgressText =>
        TotalBytes > 0
            ? $"{FormatBytes(DownloadedBytes)} / {FormatBytes(TotalBytes)} ({ProgressPercent:0.0}%)"
            : FormatBytes(DownloadedBytes);

    public string TelemetryText
    {
        get
        {
            return Status switch
            {
                "DOWNLOADING" => $"⚡ {FormatBytes(SpeedBytesPerSec)}/s • ETA {FormatEta(DownloadedBytes, TotalBytes, SpeedBytesPerSec)} {SegmentMapText}",
                "COMPLETED" => string.IsNullOrEmpty(Sha256Hash)
                    ? $"✓ Saved to {SavedFilePath}"
                    : $"✓ Saved • SHA-256: {Sha256Hash[..16]}...",
                "FAILED" => $"⚠ {ErrorMessage ?? "Transfer interrupted"}",
                _ => $"Paused at {FormatBytes(DownloadedBytes)}"
            };
        }
    }

    public static string FormatBytes(long bytes)
    {
        if (bytes <= 0) return "0 B";
        double kb = bytes / 1024.0;
        double mb = kb / 1024.0;
        double gb = mb / 1024.0;
        if (gb >= 1.0) return $"{gb:0.00} GB";
        if (mb >= 1.0) return $"{mb:0.0} MB";
        if (kb >= 1.0) return $"{kb:0} KB";
        return $"{bytes} B";
    }

    private static string FormatEta(long downloaded, long total, long speed)
    {
        if (total <= 0 || speed <= 0 || downloaded >= total) return "--";
        long secs = (total - downloaded) / speed;
        if (secs < 60) return $"{secs}s";
        if (secs < 3600) return $"{secs / 60}m {secs % 60}s";
        return $"{secs / 3600}h {(secs % 3600) / 60}m";
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    protected void OnPropertyChanged([CallerMemberName] string? name = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

public static class QueueStorage
{
    private static readonly string AppDataDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "ShunnoGet"
    );

    private static readonly string QueueFilePath = Path.Combine(AppDataDir, "shunnoget_queue.json");

    public static string PartsDirectory
    {
        get
        {
            string dir = Path.Combine(AppDataDir, "parts");
            Directory.CreateDirectory(dir);
            return dir;
        }
    }

    public static string GetTargetFolder(string category)
    {
        string userDownloads = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            "Downloads",
            "ShunnoGet",
            category
        );
        Directory.CreateDirectory(userDownloads);
        return userDownloads;
    }

    public static List<DownloadItem> LoadQueue()
    {
        try
        {
            if (!File.Exists(QueueFilePath)) return new List<DownloadItem>();
            string json = File.ReadAllText(QueueFilePath);
            var items = JsonSerializer.Deserialize<List<DownloadItem>>(json) ?? new List<DownloadItem>();
            foreach (var item in items)
            {
                if (item.Status == "DOWNLOADING")
                {
                    item.Status = "PAUSED";
                    item.SpeedBytesPerSec = 0;
                }
            }
            return items;
        }
        catch
        {
            return new List<DownloadItem>();
        }
    }

    public static void SaveQueue(IEnumerable<DownloadItem> items)
    {
        try
        {
            Directory.CreateDirectory(AppDataDir);
            string json = JsonSerializer.Serialize(items, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(QueueFilePath, json);
        }
        catch
        {
            // Ignore write contention
        }
    }
}
