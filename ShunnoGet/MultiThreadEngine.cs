using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;

namespace ShunnoGet;

public static class MultiThreadEngine
{
    private static readonly HttpClient Client = CreateHttpClient();
    private static readonly ConcurrentDictionary<long, CancellationTokenSource> ActiveTasks = new();

    // Global speed limit in bytes/sec (0 = Unlimited)
    public static long MaxBytesPerSecond { get; set; } = 0;

    private static HttpClient CreateHttpClient()
    {
        var handler = new SocketsHttpHandler
        {
            MaxConnectionsPerServer = 64,
            AutomaticDecompression = DecompressionMethods.All,
            PooledConnectionLifetime = TimeSpan.FromMinutes(10),
            ConnectTimeout = TimeSpan.FromSeconds(20)
        };
        var client = new HttpClient(handler)
        {
            Timeout = Timeout.InfiniteTimeSpan
        };
        client.DefaultRequestHeaders.UserAgent.ParseAdd("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ShunnoGet/1.0");
        return client;
    }

    public static void PauseTask(long id)
    {
        if (ActiveTasks.TryRemove(id, out var cts))
        {
            cts.Cancel();
        }
    }

    public static void StartTask(DownloadItem item, Action onQueueUpdated)
    {
        PauseTask(item.Id);
        var cts = new CancellationTokenSource();
        ActiveTasks[item.Id] = cts;

        item.Status = "DOWNLOADING";
        item.ErrorMessage = null;
        onQueueUpdated();

        Task.Run(async () =>
        {
            try
            {
                string partFile = Path.Combine(QueueStorage.PartsDirectory, $"{item.Id}_{item.FileName}.part");

                if (item.Url.Contains(".m3u8", StringComparison.OrdinalIgnoreCase))
                {
                    await DownloadHlsStreamAsync(item, partFile, cts.Token, onQueueUpdated);
                }
                else
                {
                    await DownloadMultiThreadedAsync(item, partFile, cts.Token, onQueueUpdated);
                }
            }
            catch (OperationCanceledException)
            {
                Application.Current.Dispatcher.Invoke(() =>
                {
                    item.Status = "PAUSED";
                    item.SpeedBytesPerSec = 0;
                    onQueueUpdated();
                });
            }
            catch (Exception ex)
            {
                Application.Current.Dispatcher.Invoke(() =>
                {
                    item.Status = "FAILED";
                    item.SpeedBytesPerSec = 0;
                    item.ErrorMessage = ex.Message;
                    onQueueUpdated();
                });
            }
            finally
            {
                ActiveTasks.TryRemove(item.Id, out _);
            }
        });
    }

    private static async Task DownloadMultiThreadedAsync(
        DownloadItem item,
        string partFile,
        CancellationToken token,
        Action onQueueUpdated)
    {
        long contentLength = -1;
        bool acceptRanges = false;

        try
        {
            using var headReq = new HttpRequestMessage(HttpMethod.Head, item.Url);
            ApplyCustomHeaders(headReq, item);
            using var headResp = await Client.SendAsync(headReq, HttpCompletionOption.ResponseHeadersRead, token);
            contentLength = headResp.Content.Headers.ContentLength ?? -1;
            acceptRanges = headResp.Headers.AcceptRanges.Contains("bytes") || headResp.StatusCode == HttpStatusCode.PartialContent;
        }
        catch
        {
            // Fallback probe via ranged GET(0-0)
        }

        if (contentLength <= 0)
        {
            try
            {
                using var probeReq = new HttpRequestMessage(HttpMethod.Get, item.Url);
                probeReq.Headers.Range = new RangeHeaderValue(0, 0);
                ApplyCustomHeaders(probeReq, item);
                using var probeResp = await Client.SendAsync(probeReq, HttpCompletionOption.ResponseHeadersRead, token);
                if (probeResp.StatusCode == HttpStatusCode.PartialContent)
                {
                    acceptRanges = true;
                    contentLength = probeResp.Content.Headers.ContentRange?.Length ?? -1;
                }
                else
                {
                    contentLength = probeResp.Content.Headers.ContentLength ?? -1;
                }
            }
            catch
            {
                // Ignore probe error and proceed with single stream
            }
        }

        bool useMultiThread = acceptRanges && contentLength > 512 * 1024;
        int threads = useMultiThread ? Math.Clamp(item.ThreadCount, 1, 32) : 1;

        Application.Current.Dispatcher.Invoke(() =>
        {
            item.TotalBytes = contentLength;
            item.SupportsRanges = useMultiThread;
            onQueueUpdated();
        });

        if (useMultiThread && contentLength > 0)
        {
            using (var preAlloc = new FileStream(partFile, FileMode.OpenOrCreate, FileAccess.Write, FileShare.ReadWrite))
            {
                if (preAlloc.Length != contentLength)
                {
                    preAlloc.SetLength(contentLength);
                }
            }

            long chunkSize = contentLength / threads;
            long[] threadProgress = new long[threads];
            long[] threadTotals = new long[threads];

            for (int i = 0; i < threads; i++)
            {
                long start = i * chunkSize;
                long end = (i == threads - 1) ? contentLength - 1 : (start + chunkSize - 1);
                threadTotals[i] = end - start + 1;

                string offsetFile = Path.Combine(QueueStorage.PartsDirectory, $"{item.Id}_t{i}.offset");
                if (File.Exists(offsetFile) && long.TryParse(File.ReadAllText(offsetFile).Trim(), out long savedOffset))
                {
                    long done = Math.Clamp(savedOffset - start, 0, threadTotals[i]);
                    threadProgress[i] = done;
                }
            }

            using var speedCts = CancellationTokenSource.CreateLinkedTokenSource(token);
            var monitorTask = Task.Run(async () =>
            {
                long lastTotal = threadProgress.Sum();
                while (!speedCts.Token.IsCancellationRequested)
                {
                    await Task.Delay(500, speedCts.Token);
                    long currentTotal = threadProgress.Sum();
                    long speed = Math.Max(0, (currentTotal - lastTotal) * 2);
                    lastTotal = currentTotal;

                    // Build visual segment telemetry bar
                    var segChars = threadProgress.Select((done, idx) =>
                    {
                        double ratio = threadTotals[idx] > 0 ? (double)done / threadTotals[idx] : 1.0;
                        return ratio >= 0.99 ? '█' : ratio >= 0.5 ? '▓' : ratio > 0.05 ? '▒' : '░';
                    });
                    string segBar = $"[{new string(segChars.ToArray())}]";

                    Application.Current.Dispatcher.Invoke(() =>
                    {
                        item.DownloadedBytes = currentTotal;
                        item.SpeedBytesPerSec = speed;
                        item.SegmentMapText = segBar;
                    });
                }
            }, speedCts.Token);

            var tasks = Enumerable.Range(0, threads).Select(async threadIdx =>
            {
                long startByte = threadIdx * chunkSize;
                long endByte = (threadIdx == threads - 1) ? contentLength - 1 : (startByte + chunkSize - 1);
                string offsetFile = Path.Combine(QueueStorage.PartsDirectory, $"{item.Id}_t{threadIdx}.offset");

                long currentOffset = startByte + threadProgress[threadIdx];
                if (currentOffset > endByte) return;

                using var req = new HttpRequestMessage(HttpMethod.Get, item.Url);
                req.Headers.Range = new RangeHeaderValue(currentOffset, endByte);
                ApplyCustomHeaders(req, item);

                using var resp = await Client.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, token);
                resp.EnsureSuccessStatusCode();

                using var netStream = await resp.Content.ReadAsStreamAsync(token);
                using var fileHandle = File.OpenHandle(partFile, FileMode.Open, FileAccess.Write, FileShare.ReadWrite, FileOptions.Asynchronous);

                byte[] buffer = new byte[32 * 1024];
                int bytesSinceCheckpoint = 0;

                while (true)
                {
                    token.ThrowIfCancellationRequested();
                    int read = await netStream.ReadAsync(buffer.AsMemory(0, buffer.Length), token);
                    if (read <= 0) break;

                    await RandomAccess.WriteAsync(fileHandle, buffer.AsMemory(0, read), currentOffset, token);
                    currentOffset += read;
                    Interlocked.Add(ref threadProgress[threadIdx], read);
                    bytesSinceCheckpoint += read;

                    if (bytesSinceCheckpoint >= 128 * 1024)
                    {
                        await File.WriteAllTextAsync(offsetFile, currentOffset.ToString(), token);
                        bytesSinceCheckpoint = 0;
                    }

                    // Apply Token-Bucket Bandwidth Speed Limiter if enabled
                    long limit = MaxBytesPerSecond;
                    if (limit > 0)
                    {
                        long perThreadRate = Math.Max(4096, limit / threads);
                        int delayMs = (int)((read * 1000L) / perThreadRate);
                        if (delayMs > 5) await Task.Delay(delayMs, token);
                    }
                }

                await File.WriteAllTextAsync(offsetFile, currentOffset.ToString(), token);
            });

            await Task.WhenAll(tasks);
            speedCts.Cancel();

            for (int i = 0; i < threads; i++)
            {
                string offsetFile = Path.Combine(QueueStorage.PartsDirectory, $"{item.Id}_t{i}.offset");
                if (File.Exists(offsetFile)) File.Delete(offsetFile);
            }
        }
        else
        {
            // Single-stream fallback
            using var req = new HttpRequestMessage(HttpMethod.Get, item.Url);
            ApplyCustomHeaders(req, item);
            using var resp = await Client.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, token);
            resp.EnsureSuccessStatusCode();

            long total = resp.Content.Headers.ContentLength ?? -1;
            Application.Current.Dispatcher.Invoke(() => item.TotalBytes = total);

            using var netStream = await resp.Content.ReadAsStreamAsync(token);
            using var outStream = new FileStream(partFile, FileMode.Create, FileAccess.Write, FileShare.None);

            byte[] buffer = new byte[32 * 1024];
            long downloaded = 0;
            long lastTick = Environment.TickCount64;
            long bytesSinceTick = 0;

            while (true)
            {
                token.ThrowIfCancellationRequested();
                int read = await netStream.ReadAsync(buffer.AsMemory(0, buffer.Length), token);
                if (read <= 0) break;

                await outStream.WriteAsync(buffer.AsMemory(0, read), token);
                downloaded += read;
                bytesSinceTick += read;

                long now = Environment.TickCount64;
                if (now - lastTick >= 500)
                {
                    long speed = (bytesSinceTick * 1000L) / Math.Max(1, now - lastTick);
                    lastTick = now;
                    bytesSinceTick = 0;
                    Application.Current.Dispatcher.Invoke(() =>
                    {
                        item.DownloadedBytes = downloaded;
                        item.SpeedBytesPerSec = speed;
                    });
                }
            }

            Application.Current.Dispatcher.Invoke(() =>
            {
                item.DownloadedBytes = downloaded;
                if (item.TotalBytes <= 0) item.TotalBytes = downloaded;
            });
        }

        await FinalizeCompletedFileAsync(item, partFile, item.FileName, onQueueUpdated);
    }

    private static async Task DownloadHlsStreamAsync(
        DownloadItem item,
        string partFile,
        CancellationToken token,
        Action onQueueUpdated)
    {
        string manifestText = await Client.GetStringAsync(item.Url, token);
        string baseUrl = item.Url.Contains('/') ? item.Url[..(item.Url.LastIndexOf('/') + 1)] : "";

        var lines = manifestText.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
            .Select(l => l.Trim())
            .ToList();

        // If master playlist (#EXT-X-STREAM-INF), follow highest quality variant playlist
        if (lines.Any(l => l.StartsWith("#EXT-X-STREAM-INF", StringComparison.OrdinalIgnoreCase)))
        {
            string? variantUrl = lines.LastOrDefault(l => !l.StartsWith("#"));
            if (!string.IsNullOrEmpty(variantUrl))
            {
                string resolvedVariant = variantUrl.StartsWith("http", StringComparison.OrdinalIgnoreCase)
                    ? variantUrl
                    : baseUrl + variantUrl;
                manifestText = await Client.GetStringAsync(resolvedVariant, token);
                baseUrl = resolvedVariant.Contains('/') ? resolvedVariant[..(resolvedVariant.LastIndexOf('/') + 1)] : baseUrl;
                lines = manifestText.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
                    .Select(l => l.Trim())
                    .ToList();
            }
        }

        var segments = lines.Where(l => !l.StartsWith("#"))
            .Select(l => l.StartsWith("http", StringComparison.OrdinalIgnoreCase) ? l : baseUrl + l)
            .ToList();

        if (segments.Count == 0)
            throw new Exception("No media segments found in M3U8 playlist.");

        long downloaded = 0;
        using (var outStream = new FileStream(partFile, FileMode.Create, FileAccess.Write, FileShare.None))
        {
            for (int i = 0; i < segments.Count; i++)
            {
                token.ThrowIfCancellationRequested();
                byte[] segBytes = await Client.GetByteArrayAsync(segments[i], token);
                await outStream.WriteAsync(segBytes, token);
                downloaded += segBytes.Length;

                long estimatedTotal = (downloaded * segments.Count) / (i + 1);
                Application.Current.Dispatcher.Invoke(() =>
                {
                    item.DownloadedBytes = downloaded;
                    item.TotalBytes = estimatedTotal;
                    item.SegmentMapText = $"[HLS Segment {i + 1}/{segments.Count}]";
                });
            }
        }

        string outName = item.FileName.Replace(".m3u8", ".mp4", StringComparison.OrdinalIgnoreCase);
        await FinalizeCompletedFileAsync(item, partFile, outName, onQueueUpdated);
    }

    private static async Task FinalizeCompletedFileAsync(
        DownloadItem item,
        string partFile,
        string finalFileName,
        Action onQueueUpdated)
    {
        string targetFolder = QueueStorage.GetTargetFolder(item.Category);
        string destPath = Path.Combine(targetFolder, finalFileName);

        if (File.Exists(destPath))
        {
            string baseName = Path.GetFileNameWithoutExtension(finalFileName);
            string ext = Path.GetExtension(finalFileName);
            destPath = Path.Combine(targetFolder, $"{baseName}_{DateTime.Now:HHmmss}{ext}");
        }

        File.Move(partFile, destPath, overwrite: true);

        // Compute SHA-256 integrity hash
        string sha256Hex = "";
        await using (var fs = File.OpenRead(destPath))
        {
            byte[] hashBytes = await SHA256.HashDataAsync(fs);
            sha256Hex = Convert.ToHexString(hashBytes).ToLowerInvariant();
        }

        long finalSize = new FileInfo(destPath).Length;

        Application.Current.Dispatcher.Invoke(() =>
        {
            item.TotalBytes = finalSize;
            item.DownloadedBytes = finalSize;
            item.SpeedBytesPerSec = 0;
            item.SavedFilePath = destPath;
            item.Sha256Hash = sha256Hex;
            item.Status = "COMPLETED";
            onQueueUpdated();
        });
    }

    private static void ApplyCustomHeaders(HttpRequestMessage req, DownloadItem item)
    {
        if (!string.IsNullOrWhiteSpace(item.Referer))
        {
            req.Headers.TryAddWithoutValidation("Referer", item.Referer);
        }
        if (!string.IsNullOrWhiteSpace(item.Cookie))
        {
            req.Headers.TryAddWithoutValidation("Cookie", item.Cookie);
        }
    }
}
