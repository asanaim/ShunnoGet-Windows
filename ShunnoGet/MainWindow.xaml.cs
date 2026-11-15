using System;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Microsoft.Win32;

namespace ShunnoGet;

public partial class MainWindow : Window
{
    private readonly ObservableCollection<DownloadItem> _allDownloads = new();
    private readonly ObservableCollection<DownloadItem> _filteredDownloads = new();
    private string _selectedCategory = "All";
    private string _lastClipboardUrl = "";
    private readonly DispatcherTimer _clipboardTimer;
    private HttpListener? _browserListener;

    private static readonly string[] SniffExtensions =
    {
        ".zip", ".7z", ".rar", ".exe", ".msi", ".iso", ".mp4", ".mkv", ".webm",
        ".m3u8", ".flac", ".m4a", ".wav", ".mp3", ".pdf", ".apk", ".tar", ".gz"
    };

    public MainWindow()
    {
        InitializeComponent();
        DownloadsListBox.ItemsSource = _filteredDownloads;

        foreach (var saved in QueueStorage.LoadQueue())
        {
            _allDownloads.Add(saved);
        }
        ApplyCategoryFilter();

        // Start Live Clipboard Link Sniffer (checks every 1.2s)
        _clipboardTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(1200) };
        _clipboardTimer.Tick += CheckClipboardForLinks;
        _clipboardTimer.Start();

        // Start Local Browser Extension Hook on http://127.0.0.1:45780/add?url=...
        StartLocalBrowserHookServer();
    }

    private void StartLocalBrowserHookServer()
    {
        Task.Run(async () =>
        {
            try
            {
                _browserListener = new HttpListener();
                _browserListener.Prefixes.Add("http://127.0.0.1:45780/");
                _browserListener.Start();

                while (_browserListener.IsListening)
                {
                    var ctx = await _browserListener.GetContextAsync();
                    string? targetUrl = ctx.Request.QueryString["url"];
                    string? referer = ctx.Request.QueryString["referer"];
                    string? cookie = ctx.Request.QueryString["cookie"];

                    using (var resp = ctx.Response)
                    {
                        resp.Headers.Add("Access-Control-Allow-Origin", "*");
                        resp.StatusCode = 200;
                        byte[] okBytes = System.Text.Encoding.UTF8.GetBytes("OK");
                        await resp.OutputStream.WriteAsync(okBytes, 0, okBytes.Length);
                    }

                    if (!string.IsNullOrWhiteSpace(targetUrl))
                    {
                        Dispatcher.Invoke(() =>
                        {
                            Activate();
                            EnqueueUrl(targetUrl, referer, cookie);
                        });
                    }
                }
            }
            catch
            {
                // Port already in use or listener closed
            }
        });
    }

    private void CheckClipboardForLinks(object? sender, EventArgs e)
    {
        if (ClipboardMonitorToggle.IsChecked != true) return;
        try
        {
            if (!Clipboard.ContainsText()) return;
            string clip = Clipboard.GetText().Trim();
            if (string.IsNullOrEmpty(clip) || clip == _lastClipboardUrl) return;

            if (Uri.TryCreate(clip, UriKind.Absolute, out var uri) &&
                (uri.Scheme == Uri.UriSchemeHttp || uri.Scheme == Uri.UriSchemeHttps))
            {
                string cleanPath = uri.AbsolutePath.ToLowerInvariant();
                if (SniffExtensions.Any(ext => cleanPath.EndsWith(ext)))
                {
                    _lastClipboardUrl = clip;
                    if (string.IsNullOrWhiteSpace(UrlInputBox.Text))
                    {
                        UrlInputBox.Text = clip;
                        FooterStatusText.Text = $"📋 Sniffed downloadable link from clipboard: {Path.GetFileName(cleanPath)}";
                    }
                }
            }
        }
        catch
        {
            // Ignore clipboard lock contention
        }
    }

    private int GetSelectedThreads()
    {
        return ThreadsCombo.SelectedIndex switch
        {
            0 => 1,
            1 => 4,
            2 => 8,
            3 => 16,
            4 => 32,
            _ => 16
        };
    }

    private void StartDownload_Click(object sender, RoutedEventArgs e)
    {
        string rawInput = UrlInputBox.Text.Trim();
        if (string.IsNullOrEmpty(rawInput))
        {
            MessageBox.Show("Please paste a valid HTTP/HTTPS link or wildcard pattern first.", "Shunno Get", MessageBoxButton.OK, MessageBoxImage.Information);
            return;
        }

        UrlInputBox.Text = "";
        EnqueueBatchOrWildcard(rawInput);
    }

    private void EnqueueBatchOrWildcard(string input)
    {
        var lines = input.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
            .Select(l => l.Trim())
            .Where(l => !string.IsNullOrEmpty(l));

        foreach (string line in lines)
        {
            var match = Regex.Match(line, @"\[(\d+)-(\d+)\]");
            if (match.Success)
            {
                string startStr = match.Groups[1].Value;
                string endStr = match.Groups[2].Value;
                if (int.TryParse(startStr, out int start) && int.TryParse(endStr, out int end))
                {
                    int pad = startStr.Length;
                    for (int i = start; i <= Math.Min(end, start + 100); i++)
                    {
                        string expanded = line.Replace(match.Value, i.ToString().PadLeft(pad, '0'));
                        EnqueueUrl(expanded);
                    }
                    continue;
                }
            }
            EnqueueUrl(line);
        }
    }

    private void EnqueueUrl(string rawUrl, string? referer = null, string? cookie = null)
    {
        if (!Uri.TryCreate(rawUrl.Trim(), UriKind.Absolute, out var uri)) return;

        string fileName = Path.GetFileName(uri.LocalPath);
        if (string.IsNullOrWhiteSpace(fileName) || !fileName.Contains('.'))
        {
            fileName = $"download_{DateTimeOffset.UtcNow.ToUnixTimeSeconds()}.bin";
        }

        var item = new DownloadItem
        {
            Id = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() + Random.Shared.Next(1, 999),
            Url = uri.ToString(),
            FileName = Uri.UnescapeDataString(fileName),
            ThreadCount = GetSelectedThreads(),
            Category = DetectCategory(fileName),
            Referer = referer,
            Cookie = cookie
        };

        _allDownloads.Insert(0, item);
        ApplyCategoryFilter();
        SaveQueueState();

        MultiThreadEngine.StartTask(item, OnDownloadQueueUpdated);
    }

    private void OnDownloadQueueUpdated()
    {
        SaveQueueState();
        ApplyCategoryFilter();

        if (ShutdownWhenDoneToggle.IsChecked == true &&
            _allDownloads.Count > 0 &&
            _allDownloads.All(d => d.Status != "DOWNLOADING" && d.Status != "QUEUED"))
        {
            Process.Start("shutdown", "/s /t 60");
        }
    }

    private void SaveQueueState() => QueueStorage.SaveQueue(_allDownloads);

    private void ApplyCategoryFilter()
    {
        _filteredDownloads.Clear();
        var items = _selectedCategory == "All"
            ? _allDownloads
            : _allDownloads.Where(d => d.Category == _selectedCategory);

        foreach (var item in items)
        {
            _filteredDownloads.Add(item);
        }
    }

    private static string DetectCategory(string fileName)
    {
        string ext = Path.GetExtension(fileName).ToLowerInvariant();
        return ext switch
        {
            ".mp4" or ".mkv" or ".webm" or ".mov" or ".avi" or ".ts" or ".m3u8" => "Videos",
            ".flac" or ".alac" or ".m4a" or ".wav" or ".mp3" or ".ogg" or ".opus" => "Lossless Audio",
            ".zip" or ".rar" or ".7z" or ".tar" or ".gz" or ".iso" => "Archives",
            ".exe" or ".msi" or ".apk" => "Programs",
            _ => "Documents"
        };
    }

    private void CategoryFilter_Changed(object sender, RoutedEventArgs e)
    {
        if (sender is RadioButton rb && rb.Content is string cat)
        {
            _selectedCategory = cat;
            ApplyCategoryFilter();
        }
    }

    private void SpeedLimitCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        MultiThreadEngine.MaxBytesPerSecond = SpeedLimitCombo.SelectedIndex switch
        {
            1 => 10L * 1024 * 1024,
            2 => 5L * 1024 * 1024,
            3 => 2L * 1024 * 1024,
            4 => 512L * 1024,
            _ => 0L
        };
    }

    private void ImportBatchTxt_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new OpenFileDialog
        {
            Filter = "Text Files (*.txt)|*.txt|All Files (*.*)|*.*",
            Title = "Select Batch URL List (.txt)"
        };
        if (dlg.ShowDialog() == true)
        {
            string content = File.ReadAllText(dlg.FileName);
            EnqueueBatchOrWildcard(content);
        }
    }

    private void OpenDownloadsFolder_Click(object sender, RoutedEventArgs e)
    {
        string folder = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            "Downloads",
            "ShunnoGet"
        );
        Directory.CreateDirectory(folder);
        Process.Start(new ProcessStartInfo("explorer.exe", folder) { UseShellExecute = true });
    }

    private void PauseItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: DownloadItem item })
        {
            MultiThreadEngine.PauseTask(item.Id);
        }
    }

    private void ResumeItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: DownloadItem item } && item.Status != "COMPLETED")
        {
            MultiThreadEngine.StartTask(item, OnDownloadQueueUpdated);
        }
    }

    private void FixLinkItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: DownloadItem item })
        {
            string? newUrl = PromptForNewLink(item);
            if (!string.IsNullOrWhiteSpace(newUrl) && newUrl.StartsWith("http", StringComparison.OrdinalIgnoreCase))
            {
                item.Url = newUrl.Trim();
                MultiThreadEngine.StartTask(item, OnDownloadQueueUpdated);
            }
        }
    }

    private string? PromptForNewLink(DownloadItem item)
    {
        var win = new Window
        {
            Title = $"Refresh Expired Link — {item.FileName}",
            Width = 520,
            Height = 190,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Owner = this,
            ResizeMode = ResizeMode.NoResize,
            Background = System.Windows.Media.Brushes.White
        };

        var stack = new StackPanel { Margin = new Thickness(18) };
        stack.Children.Add(new TextBlock
        {
            Text = $"Paste a fresh URL or CDN mirror to resume {item.FileName} from {DownloadItem.FormatBytes(item.DownloadedBytes)}:",
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 10)
        });

        var box = new TextBox { Text = item.Url, Height = 32, VerticalContentAlignment = VerticalAlignment.Center };
        stack.Children.Add(box);

        var btn = new Button
        {
            Content = "Update Link & Resume",
            Height = 36,
            Margin = new Thickness(0, 14, 0, 0),
            Background = System.Windows.Media.Brushes.DodgerBlue,
            Foreground = System.Windows.Media.Brushes.White,
            FontWeight = FontWeights.Bold
        };
        string? result = null;
        btn.Click += (_, _) => { result = box.Text; win.DialogResult = true; };
        stack.Children.Add(btn);

        win.Content = stack;
        return win.ShowDialog() == true ? result : null;
    }

    private void ShowFileItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: DownloadItem item })
        {
            if (!string.IsNullOrEmpty(item.SavedFilePath) && File.Exists(item.SavedFilePath))
            {
                Process.Start("explorer.exe", $"/select,\"{item.SavedFilePath}\"");
            }
            else
            {
                OpenDownloadsFolder_Click(sender, e);
            }
        }
    }

    private void DeleteItem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Tag: DownloadItem item })
        {
            MultiThreadEngine.PauseTask(item.Id);
            _allDownloads.Remove(item);
            ApplyCategoryFilter();
            SaveQueueState();
        }
    }

    protected override void OnClosed(EventArgs e)
    {
        base.OnClosed(e);
        _clipboardTimer.Stop();
        try { _browserListener?.Stop(); } catch { }
    }
}
